#[path = "./lib.rs"]
mod lib;

#[derive(clap::Parser)]
pub struct Options {
    #[clap(short = 'l', long)]
    /// Show rustdoc location then exit
    locate: bool,
    #[clap(long, env = "HOST", default_value = "127.0.0.1")]
    /// Set host
    host: String,
    #[clap(short = 'p', long, env = "PORT", default_value = "8080")]
    /// Set listening port
    port: String,
    #[clap(short = 'r', long)]
    /// Use random port
    random_port: bool,
    #[clap(short = 's', long, name = "ITEM")]
    /// Search for item
    search: Option<String>,
    #[clap(short = 'o', long)]
    /// Open in browser
    open: bool,
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
            println!("Opening {}", self.link());
            self.open_browser(self.link())?
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
            println!("{}", dir)
        } else {
            println!("Serving rust doc on {}", &self.url());
            self.open()?;
            lib::serve_rustbook(&self.addr()).await?
        })
    }
}
