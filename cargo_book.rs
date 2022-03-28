#[path = "./lib.rs"]
mod lib;

#[derive(clap::Parser)]
pub struct Options {
    #[clap(short = 'l', long)]
    /// Show rustdoc location then exit
    locate: bool,
    #[clap(long, env = "HOST", default_value = "127.0.0.1")]
    /// Set host.
    host: String,
    #[clap(short = 'p', long, env = "PORT", default_value = "8080")]
    /// Set listening port
    port: String,
    #[clap(short = 'o', long)]
    /// Open in browser. TODO: unimplemented
    open: bool,
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
        if self.locate {
            let dir = lib::find_rustdoc()
                .unwrap()
                .into_os_string()
                .into_string()
                .unwrap();
            println!("{}", dir);
            return Ok(());
        }
        println!("Serving rust doc on http://{}", &self.hostport());
        Ok(lib::serve_rustbook(&self.addr()).await?)
    }
}
