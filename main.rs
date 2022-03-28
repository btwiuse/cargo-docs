use clap::{Parser, Subcommand};
use futures_util::future;
use hyper::server::Server;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response};
use hyper_staticfile::Static;
use std::path::PathBuf;

mod cargo_book;
mod cargo_docs;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
enum Executable {
    #[clap(name = "docs")]
    Docs(cargo_docs::Options),
    #[clap(name = "book")]
    Book(cargo_book::Options),
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    Ok(match Executable::parse() {
        Executable::Docs(options) => {
            options.run().await?;
        }
        Executable::Book(options) => {
            options.run().await?;
        }
    })
}
