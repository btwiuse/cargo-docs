use cargo::core::compiler::CompileMode;
use cargo::core::Workspace;
use cargo::ops::{compile, CompileOptions, DocOptions};
use cargo::util::config::Config;
use clap::{crate_version, App, Arg};
use std::path::PathBuf;

use futures_util::future;
use http::response::Builder as ResponseBuilder;
use http::{header, StatusCode};
use hyper::server::Server;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response};
use hyper_staticfile::Static;

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

#[tokio::main]
pub async fn main() -> Result<(), anyhow::Error> {
    let matches = App::new("cargo-docs")
        .version(crate_version!())
        .arg(
            Arg::with_name("dummy")
                .hidden(true)
                .possible_value("serve-doc"),
        )
        .arg(
            Arg::with_name("port")
                .long("port")
                .takes_value(true)
                .default_value("8080"),
        )
        .arg(
            Arg::with_name("manifest-path")
                .long("manifest-path")
                .takes_value(true)
                .default_value("Cargo.toml"),
        )
        .get_matches();

    let port = matches.value_of("port").unwrap();
    let addr = format!("127.0.0.1:{port}").parse()?;
    let mut manifest_path = PathBuf::from(matches.value_of("manifest-path").unwrap());
    if !manifest_path.is_absolute() {
        manifest_path = std::env::current_dir().unwrap().join(manifest_path);
    }

    let config = Config::default().expect("Error making cargo config");
    let workspace = Workspace::new(&manifest_path, &config).expect("Error making workspace");

    let options = DocOptions {
        open_result: false,
        compile_opts: CompileOptions::new(&config, CompileMode::Doc { deps: true })
            .expect("Making CompileOptions"),
    };
    println!("Generating documentation for crate...");
    // reference:
    // https://docs.rs/cargo/latest/src/cargo/ops/cargo_doc.rs.html#18-34
    let compilation = compile(&workspace, &options.compile_opts)?;
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
    println!("Serving on http://127.0.0.1:{port}");
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
