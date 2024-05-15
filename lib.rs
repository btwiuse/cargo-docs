use cargo::core::compiler::{CompileMode, Executor};
use cargo::core::{PackageId, Shell, Target, Verbosity, Workspace};
use cargo::ops::{compile_with_exec, CompileOptions};
use cargo::util::errors::CargoResult;
use cargo::util::{homedir, GlobalContext};
use cargo_util::ProcessBuilder;
use http::response::Builder as ResponseBuilder;
use http::{header, StatusCode};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_staticfile::Body;
use hyper_staticfile::Static;
use hyper_util::rt::tokio::TokioIo;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;

/// run `cargo doc` with extra args
#[allow(dead_code)]
pub async fn run_cargo_doc(args: &Vec<String>) -> std::process::ExitStatus {
    // std::io::Result<> {
    // async fn main() ->  {
    let mut cmd = tokio::process::Command::new("cargo");
    cmd.arg("doc").args(args);
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
    let mut child = tokio::process::Command::new("cargo")
        .arg("doc")
        .args(args)
        .spawn()
        .expect("failed to run `cargo doc`");
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

/// get crate info
#[allow(dead_code)]
pub fn get_crate_info(manifest_path: &PathBuf) -> Result<(String, PathBuf), anyhow::Error> {
    let mut shell = Shell::default();
    shell.set_verbosity(Verbosity::Quiet);
    let cwd = std::env::current_dir()?;
    let cargo_home_dir = homedir(&cwd).expect("Errror locating homedir");
    let config = GlobalContext::new(shell, cwd, cargo_home_dir);
    let workspace = Workspace::new(manifest_path, &config).expect("Error making workspace");

    let mut compile_opts = CompileOptions::new(
        &config,
        CompileMode::Doc {
            deps: true,
            json: false,
        },
    )
    .expect("Making CompileOptions");

    // set to Default, otherwise cargo will complain about virtual manifest:
    //
    // https://docs.rs/cargo/latest/src/cargo/core/workspace.rs.html#265-275
    // https://docs.rs/cargo/latest/src/cargo/ops/cargo_compile.rs.html#125-184
    compile_opts.spec = cargo::ops::Packages::Default;

    // reference:
    // https://docs.rs/cargo/latest/src/cargo/ops/cargo_doc.rs.html#18-34
    /// A `DefaultExecutor` calls rustc without doing anything else. It is Cargo's
    /// default behaviour.
    #[derive(Copy, Clone)]
    struct DefaultExecutor;

    impl Executor for DefaultExecutor {
        fn exec(
            &self,
            _cmd: &ProcessBuilder,
            _id: PackageId,
            _target: &Target,
            _mode: CompileMode,
            _on_stdout_line: &mut dyn FnMut(&str) -> CargoResult<()>,
            _on_stderr_line: &mut dyn FnMut(&str) -> CargoResult<()>,
        ) -> CargoResult<()> {
            // cmd.exec_with_streaming(on_stdout_line, on_stderr_line, false).map(drop)
            Ok(())
        }
    }

    let exec: Arc<dyn Executor> = Arc::new(DefaultExecutor);
    let compilation = compile_with_exec(&workspace, &compile_opts, &exec)?;
    let root_crate_names = &compilation.root_crate_names;
    let crate_doc_dir = workspace.target_dir().join("doc").into_path_unlocked();
    let crate_name = root_crate_names
        .get(0)
        .ok_or_else(|| anyhow::anyhow!("no crates with documentation"))?;
    Ok((crate_name.to_string(), crate_doc_dir))
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
