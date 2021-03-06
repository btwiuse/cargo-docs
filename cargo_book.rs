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
    /// Show index page to all books
    all: bool,
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
    /// Book to read, use `--list` to see available books
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

struct Row(Book);

impl ToString for Row {
    fn to_string(&self) -> String {
        use strum::EnumMessage;
        format!("{: <16} {}", self.0, self.0.get_documentation().unwrap()).into()
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
        Ok(port_selector::random_free_tcp_port().expect("Error allocating free port"))
    }
    fn hostport(&self) -> String {
        format!("{}:{}", self.host(), self.port())
    }
    fn url(&self) -> String {
        format!("http://{}", self.hostport())
    }
    fn book_link(&self) -> String {
        if let Some(ref book) = self.book {
            if *book == Book::Rustlings {
                return "https://github.com/rust-lang/rustlings".to_string();
            }
            if *book == Book::RustDevGuide {
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
            log::info!("Opening {link}");
            self.open_browser(link)?
        })
    }
    fn open_browser<P: AsRef<std::ffi::OsStr>>(&self, path: P) -> Result<(), anyhow::Error> {
        Ok(opener::open_browser(path)?)
    }
    pub async fn run(&mut self) -> Result<(), anyhow::Error> {
        if self.random_port {
            self.port = format!("{}", self.get_port()?);
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
            if self.book.is_none() && !self.all {
                use dialoguer::console::Term;
                use dialoguer::{theme::ColorfulTheme, Select};
                use strum::IntoEnumIterator;
                let books: Vec<Row> = Book::iter().map(|x| Row(x)).collect();
                let selection = Select::with_theme(&ColorfulTheme::default())
                    .items(&books)
                    .default(0)
                    .interact_on_opt(&Term::stderr())?;
                match selection {
                    Some(index) => self.book = Some(books[index].0),
                    None => return Ok(()),
                }
            }
            log::info!("Serving rust doc on {}", &self.url());
            self.open()?;
            lib::serve_rustbook(&self.addr()).await?
        })
    }
}
