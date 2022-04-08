#[path = "./lib.rs"]
mod lib;

#[derive(clap::Parser)]
#[clap(
    author,
    version,
    about,
    long_about = None
)]
pub struct Options {
    #[clap(long)]
    /// Select a book interactively
    skim: bool,
    #[clap(long)]
    /// List available books
    list: bool,
    #[clap(long)]
    /// Show rustdoc location then exit
    locate: bool,
    #[clap(long, env = "HOST", default_value = "127.0.0.1")]
    /// Set host
    host: String,
    #[clap(short = 'p', long, env = "PORT", default_value = "8080")]
    /// Set listening port
    port: String,
    #[clap(short = 'r', long, env = "CARGO_BOOK_RANDOM_PORT")]
    /// Use random port
    random_port: bool,
    #[clap(short = 's', long, name = "ITEM")]
    /// Search for item
    search: Option<String>,
    #[clap(short = 'o', long, env = "CARGO_BOOK_OPEN")]
    /// Open in browser
    open: bool,
    /// Book to read, use `--list` or `--skim` to see available books
    book: Option<Book>,
}

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    clap::Parser,
    strum::EnumIter,
    strum::EnumString,
    strum::Display,
    strum::EnumMessage,
)]
enum Book {
    /// [Learn Rust] The Rust Programming Language
    #[strum(serialize = "book")]
    Rust,
    /// [Learn Rust] Rust By Example
    #[strum(serialize = "rust-by-example")]
    RustByExample,
    /// [Learn Rust] Rustlings <https://github.com/rust-lang/rustlings>
    #[strum(serialize = "rustlings")]
    Rustlings,
    /// [Use Rust] The Standard Library
    #[strum(serialize = "std")]
    Std,
    /// [Use Rust] The Edition Guide
    #[strum(serialize = "edition-guide")]
    EditionGuide,
    /// [Use Rust] The Rustc Book
    #[strum(serialize = "rustc")]
    Rustc,
    /// [Use Rust] The Cargo Book
    #[strum(serialize = "cargo")]
    Cargo,
    /// [Use Rust] The Rustdoc Book
    #[strum(serialize = "rustdoc")]
    Rustdoc,
    /// [Master Rust] The Reference
    #[strum(serialize = "reference")]
    Reference,
    /// [Master Rust] The Rustonomicon
    #[strum(serialize = "nomicon")]
    Nomicon,
    /// [Master Rust] The Unstable Book
    #[strum(serialize = "unstable-book")]
    UnstableBook,
    /// [Master Rust] The Rustc Contribution Guide <https://rustc-dev-guide.rust-lang.org>
    #[strum(serialize = "rust-dev-guide")]
    RustDevGuide,
    /// [Specialize Rust] The Embedded Rust Book
    #[strum(serialize = "embedded-book")]
    EmbeddedBook,
}

impl skim::SkimItem for Book {
    fn text(&self) -> skim::prelude::Cow<str> {
        skim::prelude::Cow::from(self.to_string())
    }
    fn display(&self, _: skim::DisplayContext) -> skim::AnsiString {
        use strum::EnumMessage;
        format!("{: <16} {}", self, self.get_documentation().unwrap()).into()
    }
}

impl Options {
    fn host(&self) -> String {
        self.host.clone()
    }
    fn port(&self) -> String {
        self.port.clone()
    }
    fn get_port(&self) -> std::io::Result<u16> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        Ok(listener.local_addr()?.port())
    }
    fn hostport(&self) -> String {
        format!("{}:{}", self.host(), self.port())
    }
    fn url(&self) -> String {
        format!("http://{}", self.hostport())
    }
    fn book_link(&self) -> String {
        if let Some(book) = self.book.clone() {
            if book == Book::Rustlings {
                return "https://github.com/rust-lang/rustlings".to_string();
            }
            if book == Book::RustDevGuide {
                return "https://rustc-dev-guide.rust-lang.org".to_string();
            }
            if self.search.is_none() {
                format!("{}/{book}", self.url())
            } else {
                format!(
                    "{}/{book}/?search={}",
                    self.url(),
                    self.search.as_ref().unwrap()
                )
            }
        } else {
            self.link()
        }
    }
    fn link(&self) -> String {
        if self.search.is_none() {
            format!("{}", self.url())
        } else {
            format!(
                "{}/std/?search={}",
                self.url(),
                self.search.as_ref().unwrap()
            )
        }
    }
    fn addr(&self) -> std::net::SocketAddr {
        self.hostport().parse().unwrap()
    }
    fn open(&self) -> Result<(), anyhow::Error> {
        Ok(if self.open {
            let link = self.book_link();
            println!("Opening {link}");
            self.open_browser(link)?
        })
    }
    fn open_browser<P: AsRef<std::ffi::OsStr>>(&self, path: P) -> Result<(), anyhow::Error> {
        Ok(opener::open_browser(path)?)
    }
    pub async fn run(&mut self) -> Result<(), anyhow::Error> {
        if self.random_port {
            self.port = format!("{}", self.get_port().unwrap());
        }
        Ok(if self.locate {
            let dir = lib::find_rustdoc()
                .unwrap()
                .into_os_string()
                .into_string()
                .unwrap();
            println!("{dir}")
        } else if self.list {
            use strum::EnumMessage;
            use strum::IntoEnumIterator;
            for book in Book::iter() {
                println!("{: <16} {}", book, book.get_documentation().unwrap());
            }
        } else {
            if self.skim {
                use strum::IntoEnumIterator;
                if let Some(book) = select_one(Book::iter()) {
                    self.book = Some(book.text().to_string().parse()?);
                } else {
                    return Ok(());
                }
            }
            println!("Serving rust doc on {}", &self.url());
            self.open()?;
            lib::serve_rustbook(&self.addr()).await?
        })
    }
}

/// select one from an iterator of SkimItem
///
/// <https://github.com/DanSnow/github-release-download/blob/99d876ca3acc869d13e3dda0c0558cef6ad4acc5/src/bin/grd.rs>
use std::sync::Arc;
pub fn select_one<T: skim::SkimItem, I: IntoIterator<Item = T>>(
    items: I,
) -> Option<Arc<dyn skim::SkimItem>> {
    let items = items
        .into_iter()
        .map(|x| Arc::new(x) as Arc<dyn skim::SkimItem>)
        .collect::<Vec<Arc<dyn skim::SkimItem>>>();
    let (tx, rx) = skim::prelude::bounded(32);
    std::thread::spawn(move || {
        for item in items {
            if tx.send(item).is_err() {
                break;
            }
        }
    });
    let items = skim::Skim::run_with(&skim::SkimOptions::default(), Some(rx));
    if let Some(ref inner) = items {
        if inner.is_abort {
            return None;
        }
    }
    let items = items
        .map(|output| output.selected_items)
        .unwrap_or_else(Vec::new);
    items.into_iter().next()
}
