



use clap::{crate_version, App, Arg};
use std::path::PathBuf;

use futures_util::future;


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

/// https://github.com/stephank/hyper-staticfile/blob/HEAD/examples/doc_server.rs
pub async fn handle_request<B>(
    req: Request<B>,
    static_: Static,
) -> Result<Response<Body>, std::io::Error> {
    static_.clone().serve(req).await
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
    }).unwrap();

    println!("rustup_dir = {rustup_dir:?}"); // Some("/home/aaron/.rustup/toolchains/nightly-2021-12-13-x86_64-unknown-linux-gnu/share/doc/rust/html")
    println!("Serving on http://127.0.0.1:{port}");
    let handler = make_service_fn(|_| {
        let rustup_dir = Static::new(rustup_dir.clone());
        future::ok::<_, hyper::Error>(service_fn(move |req| {
            handle_request(req, rustup_dir.clone())
        }))
    });
    Server::bind(&addr).serve(handler).await?;
    Ok(())
}
