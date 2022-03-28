use clap::{Parser, Subcommand};
use futures_util::future;
use hyper::server::Server;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response};
use hyper_staticfile::Static;
use std::path::PathBuf;

#[path = "./lib.rs"]
mod lib;

#[derive(Parser)]
pub struct Options {
    #[clap(long, env = "HOST", default_value = "127.0.0.1")]
    /// Set host.
    host: String,
    #[clap(short = 'p', long, env = "PORT", default_value = "8080")]
    /// Set port.
    port: String,
    #[clap(short = 'd', long, env = "DIR")]
    /// Serve directory content.
    dir: Option<PathBuf>,
    #[clap(short = 'c', long, default_value = "Cargo.toml")]
    /// Crate manifest path.
    manifest_path: String,
    #[clap(short = 'w', long)]
    /// Re-generate doc on change. TODO: unimplemented
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
    fn host(&self) -> String {
        self.host.clone()
    }
    fn port(&self) -> String {
        self.port.clone()
    }
    fn hostport(&self) -> String {
        format!("{}:{}", self.host(), self.port())
    }
    fn addr(&self) -> std::net::SocketAddr {
        self.hostport().parse().unwrap()
    }
    pub async fn run(&self) -> Result<(), anyhow::Error> {
        let hostport = self.hostport();
        Ok(if let Some(dir) = self.dir.clone() {
            let content = dir.into_os_string().into_string().unwrap();
            println!("Serving {content} on http://{hostport}");
            lib::serve_dir(&self.addr(), &self.dir.clone().unwrap()).await?
        } else if self.book {
            let content = "rust doc";
            println!("Serving {content} on http://{hostport}");
            lib::serve_rust_doc(&self.addr()).await?
        } else {
            let content = "crate doc";
            lib::run_cargo_doc(&self.extra_args);
            println!("Serving {content} on http://{hostport}");
            lib::serve_crate_doc(&self.manifest_path(), &self.addr()).await?
        })
    }
    fn manifest_path(&self) -> PathBuf {
        let mut manifest_path = PathBuf::from(&self.manifest_path);
        if !manifest_path.is_absolute() {
            manifest_path = std::env::current_dir().unwrap().join(manifest_path);
        }
        manifest_path
    }
}
