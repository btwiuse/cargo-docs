mod cargo_book;
mod cargo_docs;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[clap(bin_name = "cargo")]
struct Executable {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[clap(name = "docs")]
    #[clap(author, version, about, long_about = None)]
    Docs(cargo_docs::Options),
    #[clap(name = "book")]
    #[clap(author, version, about, long_about = None)]
    Book(cargo_book::Options),
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    lg::info::init()?;
    Ok(match Executable::parse().command {
        Command::Docs(mut options) => {
            options.run().await?;
        }
        Command::Book(mut options) => {
            options.run().await?;
        }
    })
}
