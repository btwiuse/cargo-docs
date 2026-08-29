use bytes::Bytes;
use cargo::core::{Shell, Target, Verbosity, Workspace};
use cargo::util::{homedir, GlobalContext};
use http::response::Builder as ResponseBuilder;
use http::{header, StatusCode};
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_staticfile::Body;
use hyper_staticfile::Static;
use hyper_util::rt::tokio::TokioIo;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;

/// Body type used by the watch-mode and `--all` handlers.
///
/// Using a boxed body allows mixing cheaply-constructed in-memory responses
/// (e.g. `/_buildid`, injected HTML) with the normal staticfile body in one
/// unified return type.
type DynBody = BoxBody<Bytes, std::io::Error>;

/// Wrap owned bytes in a [`DynBody`] with no I/O error path.
fn full_body(bytes: impl Into<Bytes>) -> DynBody {
    Full::new(bytes.into())
        .map_err(|e| -> std::io::Error { match e {} })
        .boxed()
}

/// JavaScript snippet injected into HTML pages when watch mode is active.
/// It polls `/_buildid` every second and reloads if the build ID has changed.
const RELOAD_SCRIPT: &str = r#"<script>
(function() {
  var lastId = null;
  function poll() {
    fetch('/_buildid')
      .then(function(r) { return r.text(); })
      .then(function(id) {
        if (lastId !== null && id !== lastId) { location.reload(); }
        lastId = id;
      })
      .catch(function() {})
      .finally(function() { setTimeout(poll, 1000); });
  }
  poll();
})();
</script>
"#;

/// the cargo to re-invoke.
///
/// Cargo exports `CARGO` as the path of the binary it is running from before
/// handing off to a custom subcommand, so honouring it keeps the whole run on
/// one cargo: the toolchain a `+nightly` or a rustup override selected, and any
/// wrapper standing in front of it. Resolving the bare name through `PATH`
/// instead may find a different one, and the two would then disagree about
/// where artifacts live.
fn cargo_binary() -> std::ffi::OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| std::ffi::OsString::from("cargo"))
}

/// run `cargo doc` with extra args
#[allow(dead_code)]
pub async fn run_cargo_doc(args: &Vec<String>) -> std::process::ExitStatus {
    let mut cmd = tokio::process::Command::new(cargo_binary());
    cmd.arg("doc").args(args);
    {
        let stdcmd = cmd.as_std();
        log::info!(
            "Running {} {}",
            stdcmd.get_program().to_string_lossy(),
            stdcmd
                .get_args()
                .map(|s| s.to_string_lossy().to_string())
                .collect::<Vec<String>>()
                .join(" ")
        );
    }
    let mut child = cmd.spawn().expect("failed to run `cargo doc`");
    child.wait().await.expect("failed to wait")
}

/// handle crate doc request with redirect on `/`
///
/// <https://github.com/stephank/hyper-staticfile/blob/HEAD/examples/doc_server.rs>
#[allow(dead_code)]
pub async fn handle_crate_request<B>(
    req: Request<B>,
    static_: Static,
    crate_name: String,
) -> Result<Response<Body>, std::io::Error> {
    let target = if let Some(query) = req.uri().query() {
        format!("/{crate_name}/?{query}")
    } else {
        format!("/{crate_name}/")
    };
    match req.uri().path() {
        "/" => Ok(ResponseBuilder::new()
            .status(StatusCode::FOUND)
            .header(header::LOCATION, target)
            .body(Body::Empty)
            .expect("unable to build response")),
        _ => static_.clone().serve(req).await,
    }
}

/// serve rust book / std doc on `addr`
#[allow(dead_code)]
pub async fn serve_rust_doc(addr: &std::net::SocketAddr) -> Result<(), anyhow::Error> {
    Ok(serve_rustbook(addr).await?)
}

/// the crate names `cargo doc` documents for one package, in manifest order.
///
/// This mirrors cargo's own default-target filter for doc mode: every
/// documented target, minus a bin whose crate name a lib in the same package
/// already claims, since the two would write to one output directory.
fn documented_crate_names(targets: &[Target]) -> impl Iterator<Item = String> + '_ {
    targets.iter().filter_map(move |target| {
        let shadowed_by_lib = target.is_bin()
            && targets
                .iter()
                .any(|other| other.is_lib() && other.crate_name() == target.crate_name());

        (target.documented() && !shadowed_by_lib).then(|| target.crate_name())
    })
}

/// get crate info
///
/// Both answers come from the manifests alone. An earlier version ran a whole
/// compile through the linked `cargo` library, with an executor that discarded
/// every rustc invocation, purely to read the first entry of the resulting
/// `root_crate_names` -- so it planned a build it never intended to run.
///
/// That planning is what made this fail. Building a plan resolves artifact
/// paths against the build directory layout of the *linked* cargo, while the
/// `cargo doc` pass runs the *installed* one; when the two disagree about the
/// layout the plan looks for artifacts that were written elsewhere, and cargo
/// reports missing extern locations and unexecutable build scripts for a build
/// that had in fact just succeeded. Reading the workspace instead depends only
/// on manifest parsing, which no layout change moves.
#[allow(dead_code)]
pub fn get_crate_info(manifest_path: &PathBuf) -> Result<(String, PathBuf), anyhow::Error> {
    let mut shell = Shell::default();
    shell.set_verbosity(Verbosity::Quiet);
    let cwd = std::env::current_dir()?;
    let cargo_home_dir = homedir(&cwd).expect("Errror locating homedir");
    let config = GlobalContext::new(shell, cwd, cargo_home_dir);
    let workspace = Workspace::new(manifest_path, &config).expect("Error making workspace");

    let crate_doc_dir = workspace.target_dir().join("doc").into_path_unlocked();

    // `default_members` is the package set cargo itself selects when no `-p`
    // was given, and it answers for a virtual manifest as well as a root one.
    let crate_name = workspace
        .default_members()
        .flat_map(|package| documented_crate_names(package.targets()))
        .next()
        .ok_or_else(|| anyhow::anyhow!("no crates with documentation"))?;

    Ok((crate_name, crate_doc_dir))
}

/// serve crate doc on `addr`
#[allow(dead_code)]
pub async fn serve_crate_doc(
    manifest_path: &PathBuf,
    addr: &std::net::SocketAddr,
) -> Result<(), anyhow::Error> {
    let (crate_name, crate_doc_dir) = get_crate_info(manifest_path)?;
    let crate_doc_dir = Static::new(crate_doc_dir.clone());
    let crate_name = crate_name.clone();
    let handler =
        service_fn(move |req| handle_crate_request(req, crate_doc_dir.clone(), crate_name.clone()));

    let listener = TcpListener::bind(addr)
        .await
        .expect("Failed to create TCP listener");

    loop {
        let (tcp, _) = listener.accept().await?;
        let io = TokioIo::new(tcp);
        let service = handler.clone();
        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                println!("Failed to serve connection: {:?}", err);
            }
        });
    }
}

/// find rust book location
///
/// Some("/home/aaron/.rustup/toolchains/nightly-2021-12-13-x86_64-unknown-linux-gnu/share/doc/rust/html")
pub fn find_rustdoc() -> Option<PathBuf> {
    let output = std::process::Command::new("rustup")
        .arg("which")
        .arg("rustdoc")
        .output()
        .ok()?;
    if output.status.success() {
        Some(PathBuf::from(String::from_utf8(output.stdout).ok()?))
    } else {
        None
    }
    .and_then(|rustdoc| {
        Some(
            rustdoc
                .parent()?
                .parent()?
                .join("share")
                .join("doc")
                .join("rust")
                .join("html"),
        )
    })
}

/// static request handler
///
/// <https://github.com/stephank/hyper-staticfile/blob/HEAD/examples/doc_server.rs>
#[allow(dead_code)]
pub async fn handle_request<B>(
    req: Request<B>,
    static_: Static,
) -> Result<Response<Body>, std::io::Error> {
    static_.clone().serve(req).await
}

/// serve rust book on `addr`
#[allow(dead_code)]
pub async fn serve_rustbook(addr: &std::net::SocketAddr) -> Result<(), anyhow::Error> {
    let rustdoc_dir = find_rustdoc().expect("Error locating rustdoc");
    Ok(serve_dir(&rustdoc_dir, addr).await?)
}

/// serve `dir` on `addr`
#[allow(dead_code)]
pub async fn serve_dir(dir: &PathBuf, addr: &std::net::SocketAddr) -> Result<(), anyhow::Error> {
    let dir = Static::new(dir.clone());
    let handler = service_fn(move |req| handle_request(req, dir.clone()));

    let listener = TcpListener::bind(addr)
        .await
        .expect("Failed to create TCP listener");

    loop {
        let (tcp, _) = listener.accept().await?;
        let io = TokioIo::new(tcp);
        let service = handler.clone();
        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                println!("Failed to serve connection: {:?}", err);
            }
        });
    }
}

/// handle crate doc request in watch mode.
///
/// Responds to `/_buildid` with the current build counter so the injected
/// JavaScript can detect when a rebuild has completed and reload the page.
/// HTML responses have `RELOAD_SCRIPT` injected before `</body>`.
#[allow(dead_code)]
pub async fn handle_crate_request_watch<B>(
    req: Request<B>,
    static_: Static,
    crate_name: String,
    build_id: Arc<AtomicU64>,
) -> Result<Response<DynBody>, std::io::Error> {
    // Serve the /_buildid endpoint used by the injected live-reload script.
    if req.uri().path() == "/_buildid" {
        let id = build_id.load(Ordering::Relaxed).to_string();
        return Ok(ResponseBuilder::new()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-cache, no-store")
            .body(full_body(id))
            .expect("unable to build response"));
    }

    let target = if let Some(query) = req.uri().query() {
        format!("/{crate_name}/?{query}")
    } else {
        format!("/{crate_name}/")
    };
    if req.uri().path() == "/" {
        return Ok(ResponseBuilder::new()
            .status(StatusCode::FOUND)
            .header(header::LOCATION, target)
            .body(full_body(Bytes::new()))
            .expect("unable to build response"));
    }

    let response = static_.clone().serve(req).await?;

    let status = response.status();

    // If the file is missing (e.g. during a rebuild), serve a minimal HTML page
    // with the reload script so the browser keeps polling /_buildid and
    // eventually reloads when the build completes. Without this the browser
    // would land on a script-less 404 and never recover.
    if status == StatusCode::NOT_FOUND {
        let fallback = format!(
            r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Rebuilding…</title></head><body><p>Regenerating documentation…</p>{}</body></html>"#,
            RELOAD_SCRIPT
        );
        let fallback_bytes = Bytes::from(fallback);
        return Ok(ResponseBuilder::new()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-cache, no-store")
            .body(full_body(fallback_bytes))
            .expect("unable to build response"));
    }

    // Inject the live-reload script into HTML responses.
    let is_html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("text/html"))
        .unwrap_or(false);

    if is_html {
        let (mut parts, body) = response.into_parts();
        let bytes = body
            .collect()
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
            .to_bytes();
        let html = String::from_utf8_lossy(&bytes);
        let modified: String = if html.contains("</body>") {
            html.replacen("</body>", RELOAD_SCRIPT, 1) + "</body>"
        } else {
            html.into_owned() + RELOAD_SCRIPT
        };
        let modified_bytes = Bytes::from(modified);
        parts.headers.insert(
            header::CONTENT_LENGTH,
            modified_bytes.len().into(),
        );
        return Ok(Response::from_parts(parts, full_body(modified_bytes)));
    }

    Ok(response.map(|body| body.boxed()))
}

/// serve crate doc in watch mode on `addr`.
///
/// Uses [`handle_crate_request_watch`] which injects a live-reload script
/// into HTML pages and serves the `/_buildid` endpoint.
#[allow(dead_code)]
pub async fn serve_crate_doc_watch(
    manifest_path: &PathBuf,
    addr: &std::net::SocketAddr,
    build_id: Arc<AtomicU64>,
) -> Result<(), anyhow::Error> {
    let (crate_name, crate_doc_dir) = get_crate_info(manifest_path)?;
    let crate_doc_dir = Static::new(crate_doc_dir.clone());
    let crate_name = crate_name.clone();
    let handler = service_fn(move |req| {
        handle_crate_request_watch(
            req,
            crate_doc_dir.clone(),
            crate_name.clone(),
            Arc::clone(&build_id),
        )
    });

    let listener = TcpListener::bind(addr)
        .await
        .expect("Failed to create TCP listener");

    loop {
        let (tcp, _) = listener.accept().await?;
        let io = TokioIo::new(tcp);
        let service = handler.clone();
        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                log::error!("Failed to serve connection: {:?}", err);
            }
        });
    }
}

/// handle request for the `--all` index page.
///
/// Serves the provided `index_html` for the root path `/`, and delegates
/// everything else to the underlying static file server.
#[allow(dead_code)]
pub async fn handle_all_books_request<B>(
    req: Request<B>,
    static_: Static,
    index_html: Arc<String>,
) -> Result<Response<DynBody>, std::io::Error> {
    match req.uri().path() {
        "/" | "/index.html" => Ok(ResponseBuilder::new()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(full_body(index_html.as_str().to_owned()))
            .expect("unable to build response")),
        _ => Ok(static_.clone().serve(req).await?.map(|body| body.boxed())),
    }
}

/// serve rust book on `addr` with a custom HTML index page at `/`.
///
/// Used by the `cargo book --all` subcommand to present a browsable list of
/// all available books before delegating to the normal static file server.
#[allow(dead_code)]
pub async fn serve_rustbook_with_index(
    addr: &std::net::SocketAddr,
    index_html: String,
) -> Result<(), anyhow::Error> {
    let rustdoc_dir = find_rustdoc().expect("Error locating rustdoc");
    let dir = Static::new(rustdoc_dir);
    let index_html = Arc::new(index_html);
    let handler = service_fn(move |req| {
        handle_all_books_request(req, dir.clone(), Arc::clone(&index_html))
    });

    let listener = TcpListener::bind(addr)
        .await
        .expect("Failed to create TCP listener");

    loop {
        let (tcp, _) = listener.accept().await?;
        let io = TokioIo::new(tcp);
        let service = handler.clone();
        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                log::error!("Failed to serve connection: {:?}", err);
            }
        });
    }
}
