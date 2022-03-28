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
    #[clap(short = 'l', long)]
    /// Show rustdoc location then exit
    locate: bool,
    #[clap(short = 'p', long, default_value = "8080")]
    /// Set listening port
    port: String,
    #[clap(short = 'o', long)]
    /// Open in browser. TODO: unimplemented
    open: bool,
}

impl Options {
    pub async fn run(&self) -> Result<(), anyhow::Error> {
        if self.locate {
            let dir = lib::find_rustdoc()
                .unwrap()
                .into_os_string()
                .into_string()
                .unwrap();
            println!("{}", dir);
            return Ok(());
        }
        println!("Serving on http://127.0.0.1:{}", &self.port);
        let addr: std::net::SocketAddr = format!("127.0.0.1:{}", &self.port).parse()?;
        Ok(lib::serve_rustbook(&addr).await?)
    }
}
