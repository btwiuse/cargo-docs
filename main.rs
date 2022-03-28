use cargo::core::compiler::{CompileMode, Executor};
use cargo::core::{PackageId, Shell, Target, Verbosity, Workspace};
use cargo::ops::{compile_with_exec, CompileOptions};
use cargo::util::config::{homedir, Config};
use cargo::util::errors::CargoResult;
use cargo_util::ProcessBuilder;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;

use futures_util::future;
use http::response::Builder as ResponseBuilder;
use http::{header, StatusCode};
use hyper::server::Server;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response};
use hyper_staticfile::Static;

/// A `DefaultExecutor` calls rustc without doing anything else. It is Cargo's
/// default behaviour.
#[derive(Copy, Clone)]
pub struct DefaultExecutor;

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
}

/// <https://github.com/stephank/hyper-staticfile/blob/HEAD/examples/doc_server.rs>
pub async fn handle_request<B>(
    req: Request<B>,
    static_: Static,
    crate_name: String,
) -> Result<Response<Body>, std::io::Error> {
    match req.uri().path() {
        "/" => Ok(ResponseBuilder::new()
            .status(StatusCode::MOVED_PERMANENTLY)
            .header(header::LOCATION, format!("/{}/", crate_name))
            .body(Body::empty())
            .expect("unable to build response")),
        _ => static_.clone().serve(req).await,
    }
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
enum Executable {
    #[clap(name = "docs")]
    Docs(Options),
}

#[derive(Parser)]
struct Options {
    #[clap(short = 'p', long, default_value = "8080")]
    /// listening port
    port: String,
    #[clap(short = 'c', long, default_value = "Cargo.toml")]
    /// manifest path
    manifest_path: String,
    #[clap(short = 'w', long)]
    /// TODO: unimplemented
    watch: bool,
    #[clap(short = 'o', long)]
    /// TODO: unimplemented
    open: bool,
    /// passthrough extra args to `cargo doc`
    extra_args: Vec<String>,
}

#[tokio::main]
pub async fn main() -> Result<(), anyhow::Error> {
    match Executable::parse() {
        Executable::Docs(options) => {
            run(&options).await?;
        }
    }
    return Ok(());
}

fn cargo_doc(args: &Vec<String>) {
    let mut child = std::process::Command::new("cargo")
        .arg("doc")
        .args(args)
        .spawn()
        .expect("failed to run `cargo doc`");
    child.wait().expect("failed to wait");
}

async fn run(options: &Options) -> Result<(), anyhow::Error> {
    let addr = format!("127.0.0.1:{}", &options.port).parse()?;
    let mut manifest_path = PathBuf::from(&options.manifest_path);
    if !manifest_path.is_absolute() {
        manifest_path = std::env::current_dir().unwrap().join(manifest_path);
    }

    let mut shell = Shell::default();
    shell.set_verbosity(Verbosity::Quiet);
    let cwd = std::env::current_dir().unwrap();
    let cargo_home_dir = homedir(&cwd).unwrap();
    let config = Config::new(shell, cwd, cargo_home_dir);
    let workspace = Workspace::new(&manifest_path, &config).expect("Error making workspace");

    let mut compile_opts = CompileOptions::new(&config, CompileMode::Doc { deps: true })
        .expect("Making CompileOptions");

    // set to Default, otherwise cargo will complain about virtual manifest:
    //
    // https://docs.rs/cargo/latest/src/cargo/core/workspace.rs.html#265-275
    // https://docs.rs/cargo/latest/src/cargo/ops/cargo_compile.rs.html#125-184
    compile_opts.spec = cargo::ops::Packages::Default;

    // println!("Generating documentation for crate...");
    cargo_doc(&options.extra_args);
    // reference:
    // https://docs.rs/cargo/latest/src/cargo/ops/cargo_doc.rs.html#18-34
    let exec: Arc<dyn Executor> = Arc::new(DefaultExecutor);
    let compilation = compile_with_exec(&workspace, &compile_opts, &exec)?;
    let root_crate_names = &compilation.root_crate_names;
    let crate_name = root_crate_names
        .get(0)
        .ok_or_else(|| anyhow::anyhow!("no crates with documentation"))?;

    let crate_doc_dir = workspace.target_dir().join("doc").into_path_unlocked();

    println!("Serving on http://127.0.0.1:{}", options.port);
    let handler = make_service_fn(|_| {
        let crate_doc_dir = Static::new(crate_doc_dir.clone());
        let crate_name = crate_name.clone();
        future::ok::<_, hyper::Error>(service_fn(move |req| {
            handle_request(req, crate_doc_dir.clone(), crate_name.clone())
        }))
    });
    Server::bind(&addr).serve(handler).await?;
    Ok(())
}
