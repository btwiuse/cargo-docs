use cargo::core::compiler::{CompileMode, Executor};
use cargo::core::{PackageId, Target, Workspace};
use cargo::ops::{compile_with_exec, CompileOptions, DocOptions};
use cargo::util::config::Config;
use cargo::util::errors::CargoResult;
use cargo_util::ProcessBuilder;
use clap::{Parser, Subcommand};
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
    #[clap(name = "clap")]
    Clap,
    #[clap(name = "docs")]
    Docs(Docs),
    #[clap(name = "metadata", allow_hyphen_values = true)]
    Metadata(Metadata),
    #[clap(subcommand)]
    Sub(Sub),
}

#[derive(Subcommand)]
enum Sub {
    A { name: String },
}

#[derive(Parser)]
struct Metadata {
    args: Vec<String>,
}

#[derive(Parser)]
struct Docs {
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
    let cli = Executable::parse();
    match cli {
        Executable::Metadata(Metadata { args }) => {
            let mut child = std::process::Command::new("cargo")
                .arg("metadata")
                .args(&args)
                .spawn()
                .expect("failed to publish");
            child.wait().expect("failed to wait");
            // println!("{:?}", args);
        }
        Executable::Docs(docs) => {
            println!("{:?}", docs.extra_args);
            app(&docs).await?;
        }
        _ => return Ok(()),
    }
    return Ok(());
}

async fn app(docs: &Docs) -> Result<(), anyhow::Error> {
    let addr = format!("127.0.0.1:{}", &docs.port).parse()?;
    let mut manifest_path = PathBuf::from(&docs.manifest_path);
    if !manifest_path.is_absolute() {
        manifest_path = std::env::current_dir().unwrap().join(manifest_path);
    }

    let config = Config::default().expect("Error making cargo config");
    let workspace = Workspace::new(&manifest_path, &config).expect("Error making workspace");

    let mut compile_opts = CompileOptions::new(&config, CompileMode::Doc { deps: true })
        .expect("Making CompileOptions");

    // set to Default, otherwise cargo will complain about virtual manifest:
    //
    // https://docs.rs/cargo/latest/src/cargo/core/workspace.rs.html#265-275
    // https://docs.rs/cargo/latest/src/cargo/ops/cargo_compile.rs.html#125-184
    compile_opts.spec = cargo::ops::Packages::Default;

    // println!("{:?}", compile_opts.spec);

    let options = DocOptions {
        open_result: false,
        compile_opts: compile_opts,
    };
    println!("Generating documentation for crate...");
    {
        let mut child = std::process::Command::new("cargo")
            .arg("doc")
            .args(&docs.extra_args)
            .spawn()
            .expect("failed to run `cargo doc`");
        child.wait().expect("failed to wait");
    }
    // reference:
    // https://docs.rs/cargo/latest/src/cargo/ops/cargo_doc.rs.html#18-34
    let exec: Arc<dyn Executor> = Arc::new(DefaultExecutor);
    let compilation = compile_with_exec(&workspace, &options.compile_opts, &exec)?;
    let root_crate_names = &compilation.root_crate_names;
    let crate_name = root_crate_names
        .get(0)
        .ok_or_else(|| anyhow::anyhow!("no crates with documentation"))?;
    /*
    let kind = options.compile_opts.build_config.single_requested_kind()?;
    let path = compilation.root_output[&kind]
        .with_file_name("doc")
        .join(&crate_name)
        .join("index.html");
    println!("{:?}", &root_crate_names); // ["cargo_docs"]
    println!("{:?}", &crate_name); // cargo_docs
    println!("{:?}", &kind); // Host
    println!("{:?}", &path); // "/home/aaron/cargo-serve-doc/target/doc/cargo_docs/index.html"
    */
    // doc(&workspace, &options).expect("Running doc");
    // println!("{:?}", workspace);
    // println!("{:?}", options);

    let crate_doc_dir = workspace.target_dir().join("doc").into_path_unlocked();

    /*
    let rustup_dir = find_rustdoc().and_then(|rustdoc| {
        Some(
            rustdoc
                .parent()?
                .parent()?
                .join("share")
                .join("doc")
                .join("rust")
                .join("html"),
        )
    });

    println!("crate_doc_dir = {crate_doc_dir:?}"); // "/home/aaron/cargo-serve-doc/target/doc"
    println!("rustup_dir = {rustup_dir:?}"); // Some("/home/aaron/.rustup/toolchains/nightly-2021-12-13-x86_64-unknown-linux-gnu/share/doc/rust/html")
    */
    println!("Serving on http://127.0.0.1:{}", docs.port);
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
