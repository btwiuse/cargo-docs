mod cargo_book;
mod cargo_docs;

use clap::Parser;

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
    lg::info::init()?;
    Ok(match Executable::parse() {
        Executable::Docs(mut options) => {
            options.run().await?;
        }
        Executable::Book(mut options) => {
            options.run().await?;
        }
    })
}
