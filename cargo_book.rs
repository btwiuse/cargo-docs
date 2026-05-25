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

impl std::fmt::Display for Row {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use strum::EnumMessage;
        write!(f, "{: <16} {}", self.0, self.0.get_documentation().unwrap())
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
    /// Generate an HTML index page that lists all available books.
    fn generate_index_html(&self) -> String {
        use strum::EnumMessage;
        use strum::IntoEnumIterator;

        let base = self.url();
        let rows: String = Book::iter()
            .map(|book| {
                let desc = book.get_documentation().unwrap_or_default();
                // External books open directly in the browser; local books are
                // served under /{book-name}/ by the static file server.
                let href = match book {
                    Book::Rustlings => {
                        "https://github.com/rust-lang/rustlings".to_string()
                    }
                    Book::RustDevGuide => {
                        "https://rustc-dev-guide.rust-lang.org".to_string()
                    }
                    _ => format!("{base}/{book}/"),
                };
                format!(
                    r#"<tr><td><a href="{href}">{book}</a></td><td>{desc}</td></tr>"#,
                    href = href,
                    book = book,
                    desc = desc,
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Rust Books Index</title>
  <style>
    body {{ font-family: sans-serif; max-width: 800px; margin: 2rem auto; padding: 0 1rem; }}
    h1   {{ border-bottom: 1px solid #ccc; padding-bottom: .5rem; }}
    table {{ border-collapse: collapse; width: 100%; }}
    td, th {{ text-align: left; padding: .4rem .8rem; border-bottom: 1px solid #eee; }}
    th {{ background: #f5f5f5; }}
    a  {{ color: #0074d9; text-decoration: none; }}
    a:hover {{ text-decoration: underline; }}
  </style>
</head>
<body>
  <h1>Rust Books</h1>
  <table>
    <thead><tr><th>Book</th><th>Description</th></tr></thead>
    <tbody>
{rows}
    </tbody>
  </table>
</body>
</html>
"#,
            rows = rows,
        )
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
        } else if self.all {
            // Serve all books with a generated index page at /.
            let index_html = self.generate_index_html();
            log::info!("Serving all rust books on {}", &self.url());
            self.open()?;
            lib::serve_rustbook_with_index(&self.addr(), index_html).await?
        } else {
            if self.book.is_none() {
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
