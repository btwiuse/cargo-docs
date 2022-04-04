use cargo::core::compiler::{CompileMode, Executor};
use cargo::core::{PackageId, Shell, Target, Verbosity, Workspace};
use cargo::ops::{compile_with_exec, CompileOptions};
use cargo::util::config::{homedir, Config};
use cargo::util::errors::CargoResult;
use cargo_util::ProcessBuilder;
use futures_util::future;
use http::response::Builder as ResponseBuilder;
use http::{header, StatusCode};
use hyper::server::Server;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response};
use hyper_staticfile::Static;
use std::path::PathBuf;
use std::sync::Arc;

/// run `cargo doc` with extra args
#[allow(dead_code)]
pub fn run_cargo_doc(args: &Vec<String>) {
    let mut child = std::process::Command::new("cargo")
        .arg("doc")
        .args(args)
        .spawn()
        .expect("failed to run `cargo doc`");
    child.wait().expect("failed to wait");
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
    match req.uri().path() {
        "/" => Ok(ResponseBuilder::new()
            .status(StatusCode::FOUND)
            .header(header::LOCATION, format!("/{}/", crate_name))
            .body(Body::empty())
            .expect("unable to build response")),
        _ => static_.clone().serve(req).await,
    }
}

/// serve rust book / std doc on `addr`
#[allow(dead_code)]
pub async fn serve_rust_doc(addr: &std::net::SocketAddr) -> Result<(), anyhow::Error> {
    Ok(serve_rustbook(addr).await?)
}

/// serve crate doc on `addr`
#[allow(dead_code)]
pub async fn serve_crate_doc(
    manifest_path: &PathBuf,
    addr: &std::net::SocketAddr,
) -> Result<(), anyhow::Error> {
    let mut shell = Shell::default();
    shell.set_verbosity(Verbosity::Quiet);
    let cwd = std::env::current_dir().unwrap();
    let cargo_home_dir = homedir(&cwd).unwrap();
    let config = Config::new(shell, cwd, cargo_home_dir);
    let workspace = Workspace::new(manifest_path, &config).expect("Error making workspace");

    let mut compile_opts = CompileOptions::new(&config, CompileMode::Doc { deps: true })
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

    let handler = make_service_fn(|_| {
        let crate_doc_dir = Static::new(crate_doc_dir.clone());
        let crate_name = crate_name.clone();
        future::ok::<_, hyper::Error>(service_fn(move |req| {
            handle_crate_request(req, crate_doc_dir.clone(), crate_name.clone())
        }))
    });

    Ok(Server::bind(addr).serve(handler).await?)
}

/// find rust book location
// println!("rustup_dir = {rustup_dir:?}");
// Some("/home/aaron/.rustup/toolchains/nightly-2021-12-13-x86_64-unknown-linux-gnu/share/doc/rust/html")
pub fn find_rustdoc() -> Option<PathBuf> {
    let output = std::process::Command::new("rustup")
        .arg("which")
        .arg("rustdoc")
        .output()
        .unwrap();
    if output.status.success() {
        Some(PathBuf::from(String::from_utf8(output.stdout).unwrap()))
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
    let rustdoc_dir = find_rustdoc().unwrap();
    Ok(serve_dir(&rustdoc_dir, addr).await?)
}

/// serve `dir` on `addr`
#[allow(dead_code)]
pub async fn serve_dir(dir: &PathBuf, addr: &std::net::SocketAddr) -> Result<(), anyhow::Error> {
    let handler = make_service_fn(|_| {
        let dir = Static::new(dir.clone());
        future::ok::<_, hyper::Error>(service_fn(move |req| handle_request(req, dir.clone())))
    });
    Ok(Server::bind(addr).serve(handler).await?)
}
