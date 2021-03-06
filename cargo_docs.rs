#[path = "./lib.rs"]
mod lib;

use std::path::PathBuf;

#[derive(clap::Parser)]
pub struct Options {
    #[clap(long, env = "HOST", default_value = "127.0.0.1")]
    /// Set host
    host: String,
    #[clap(short = 'p', long, env = "PORT", default_value = "8080")]
    /// Set port
    port: String,
    #[clap(short = 'r', long, env = "CARGO_DOCS_RANDOM_PORT")]
    /// Use random port
    random_port: bool,
    #[clap(short = 's', long, name = "ITEM")]
    /// Search for item
    search: Option<String>,
    #[clap(short = 'd', long, env = "DIR")]
    /// Serve directory content
    dir: Option<PathBuf>,
    #[clap(short = 'c', long, default_value = "Cargo.toml")]
    /// Crate manifest path.
    manifest_path: String,
    #[clap(short = 'w', long, env = "CARGO_DOCS_WATCH")]
    /// Re-generate doc on change
    watch: bool,
    #[clap(short = 'o', long, env = "CARGO_DOCS_OPEN")]
    /// Open in browser
    open: bool,
    #[clap(short = 'b', long)]
    /// Serve rust book and std doc instead
    book: bool,
    /// Passthrough extra args to `cargo doc`
    extra_args: Vec<String>,
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
    fn link(&self) -> String {
        if self.search.is_none() {
            format!("{}", self.url())
        } else {
            if self.book {
                format!(
                    "{}/std/?search={}",
                    self.url(),
                    self.search.as_ref().unwrap()
                )
            } else {
                format!("{}/?search={}", self.url(), self.search.as_ref().unwrap())
            }
        }
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
        if !self.open {
            return Ok(());
        }
        log::info!("Opening {}", self.link());
        Ok(self.open_browser(self.link())?)
    }
    fn open_browser<P: AsRef<std::ffi::OsStr>>(&self, path: P) -> Result<(), anyhow::Error> {
        Ok(opener::open_browser(path)?)
    }
    fn watch(&self) -> Result<(), anyhow::Error> {
        if !self.watch {
            return Ok(());
        }
        log::info!("Listening for changes...");
        let (tx, mut rx) = tokio::sync::mpsc::channel(32);
        // signal listener
        let extra_args = self.extra_args.clone();
        tokio::spawn(async move {
            // let _ = lib::get_crate_info(&self.manifest_path());
            loop {
                let _msg = rx.recv().await;
                // tokio::time::sleep(tokio::time::Duration::new(1, 0)).await;
                // log::info!("Updating");
                if lib::run_cargo_doc(&extra_args).await.success() {
                    // trigger browser reload
                }
            }
        });
        // signal emitter
        tokio::spawn(async move {
            // let _ = lib::get_crate_info(&self.manifest_path());
            loop {
                tokio::time::sleep(tokio::time::Duration::new(5, 0)).await;
                tx.send(1).await.unwrap();
            }
        });
        Ok(())
    }
    pub async fn run(&mut self) -> Result<(), anyhow::Error> {
        if self.random_port {
            self.port = format!("{}", self.get_port()?);
        }
        let url = self.url();
        Ok(if let Some(dir) = self.dir.clone() {
            let content = dir.into_os_string().into_string().unwrap();
            log::info!("Serving {content} on {url}");
            lib::serve_dir(&self.dir.clone().unwrap(), &self.addr()).await?
        } else if self.book {
            let content = "rust doc";
            log::info!("Serving {content} on {url}");
            self.open()?;
            lib::serve_rust_doc(&self.addr()).await?
        } else {
            let content = "crate doc";
            if !lib::run_cargo_doc(&self.extra_args).await.success() {
                return Err(anyhow::anyhow!("failed to run cargo doc"));
            }
            self.watch()?;
            self.open()?;
            log::info!("Serving {content} on {url}");
            lib::serve_crate_doc(&self.manifest_path(), &self.addr()).await?
        })
    }
}
