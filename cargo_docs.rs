#[path = "./lib.rs"]
mod lib;

use std::path::PathBuf;

#[derive(clap::Parser)]
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
    /// Open in browser.
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
    fn url(&self) -> String {
        format!("http://{}", self.hostport())
    }
    fn addr(&self) -> std::net::SocketAddr {
        self.hostport().parse().unwrap()
    }
    fn manifest_path(&self) -> PathBuf {
        let mut manifest_path = PathBuf::from(&self.manifest_path);
        if !manifest_path.is_absolute() {
            manifest_path = std::env::current_dir().unwrap().join(manifest_path);
        }
        manifest_path
    }
    fn open(&self) -> Result<(), anyhow::Error> {
        if self.open {
            self.open_browser(self.url())?
        }
        Ok(())
    }
    fn open_browser<P: AsRef<std::ffi::OsStr>>(&self, path: P) -> Result<(), anyhow::Error> {
        Ok(opener::open_browser(path)?)
    }
    pub async fn run(&self) -> Result<(), anyhow::Error> {
        let url = self.url();
        Ok(if let Some(dir) = self.dir.clone() {
            let content = dir.into_os_string().into_string().unwrap();
            println!("Serving {content} on {url}");
            lib::serve_dir(&self.dir.clone().unwrap(), &self.addr()).await?
        } else if self.book {
            let content = "rust doc";
            println!("Serving {content} on {url}");
            self.open()?;
            lib::serve_rust_doc(&self.addr()).await?
        } else {
            let content = "crate doc";
            lib::run_cargo_doc(&self.extra_args);
            println!("Serving {content} on {url}");
            self.open()?;
            lib::serve_crate_doc(&self.manifest_path(), &self.addr()).await?
        })
    }
}
