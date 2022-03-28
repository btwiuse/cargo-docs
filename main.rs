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

mod lib;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
enum Executable {
    #[clap(name = "docs")]
    Docs(Options),
}

#[derive(Parser)]
struct Options {
    #[clap(short = 'p', long, default_value = "8080")]
    /// Set Listening port.
    port: String,
    #[clap(short = 'c', long, default_value = "Cargo.toml")]
    /// Crate manifest path.
    manifest_path: String,
    #[clap(short = 'w', long)]
    /// Re-generate doc on content change. TODO: unimplemented
    watch: bool,
    #[clap(short = 'o', long)]
    /// Open in browser. TODO: unimplemented
    open: bool,
    #[clap(short = 'b', long)]
    /// Serve rust book and std doc instead.
    book: bool,
    /// Passthrough extra args to `cargo doc`.
    extra_args: Vec<String>,
}

impl Options {
    async fn run(&self) -> Result<(), anyhow::Error> {
        let addr = format!("127.0.0.1:{}", &self.port).parse()?;
        Ok(if self.book {
            println!("Serving rust doc on http://127.0.0.1:{}", self.port);
            self.serve_rust_doc(&addr).await?
        } else {
            println!("Serving crate doc on http://127.0.0.1:{}", self.port);
            // println!("Generating documentation for crate...");
            lib::run_cargo_doc(&self.extra_args);
            lib::serve_crate_doc(&self.manifest_path(), &addr).await?
        })
    }
    async fn serve_rust_doc(&self, addr: &std::net::SocketAddr) -> Result<(), anyhow::Error> {
        Ok(cargo_book::serve_rustbook(addr).await?)
    }
    fn manifest_path(&self) -> PathBuf {
        let mut manifest_path = PathBuf::from(&self.manifest_path);
        if !manifest_path.is_absolute() {
            manifest_path = std::env::current_dir().unwrap().join(manifest_path);
        }
        manifest_path
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    Ok(match Executable::parse() {
        Executable::Docs(options) => {
            options.run().await?;
        }
    })
}
