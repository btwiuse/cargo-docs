#[path = "./lib.rs"]
mod lib;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

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
    #[clap(short = 'Z', value_name = "FLAG")]
    /// Pass unstable flag to `cargo doc` (repeatable)
    unstable_flags: Vec<String>,
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
    fn cargo_doc_args(&self) -> Vec<String> {
        let mut args = Vec::with_capacity(self.unstable_flags.len() * 2 + self.extra_args.len());
        for flag in &self.unstable_flags {
            args.push("-Z".to_owned());
            args.push(flag.clone());
        }
        args.extend(self.extra_args.clone());
        args
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
    fn watch(&self, build_id: Arc<AtomicU64>) -> Result<(), anyhow::Error> {
        if !self.watch {
            return Ok(());
        }
        log::info!("Listening for changes...");

        let cargo_doc_args = self.cargo_doc_args();
        let manifest_path = self.manifest_path();

        // Determine the source directory to watch (the directory that contains
        // the manifest, i.e. the crate root).
        let watch_dir = manifest_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap());

        // Use a std channel so notify can send events from its own thread,
        // and bridge into a tokio mpsc channel for the async rebuild task.
        let (std_tx, std_rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();
        let (tok_tx, mut tok_rx) = tokio::sync::mpsc::channel::<()>(8);

        // File-system watcher thread – runs outside the async runtime.
        let tok_tx_clone = tok_tx.clone();
        std::thread::spawn(move || {
            use notify::Watcher;
            let mut watcher = match notify::recommended_watcher(std_tx) {
                Ok(w) => w,
                Err(e) => {
                    log::error!("Failed to create file watcher: {e}");
                    return;
                }
            };
            if let Err(e) = watcher.watch(&watch_dir, notify::RecursiveMode::Recursive) {
                log::error!("Failed to watch {}: {e}", watch_dir.display());
                return;
            }
            for event in std_rx {
                match event {
                    Ok(ev) => {
                        use notify::EventKind::*;
                        let affects_non_generated_path = ev.paths.is_empty()
                            || ev.paths.iter().any(|path| {
                                let relative = path
                                    .strip_prefix(&watch_dir)
                                    .ok()
                                    .and_then(|relative| relative.components().next())
                                    .map(|component| component.as_os_str() != "target")
                                    .unwrap_or(true);
                                // Only rebuild on .rs file changes.
                                relative
                                    && path
                                        .extension()
                                        .map(|ext| ext == "rs")
                                        .unwrap_or(false)
                            });
                        if matches!(ev.kind, Modify(_) | Create(_) | Remove(_))
                            && affects_non_generated_path
                        {
                            let _ = tok_tx_clone.blocking_send(());
                        }
                    }
                    Err(e) => log::warn!("Watch error: {e}"),
                }
            }
        });

        // Async rebuild task – waits for change signals and runs `cargo doc`.
        tokio::spawn(async move {
            // Debounce: wait for the first change signal then drain any
            // additional events that were queued while we were rebuilding.
            // This prevents multiple consecutive rebuilds when many files
            // change simultaneously (e.g. after a `git checkout` or a
            // formatter run).
            while let Some(()) = tok_rx.recv().await {
                // Drain additional events that arrived while we were rebuilding.
                while tok_rx.try_recv().is_ok() {}

                log::info!("Change detected – regenerating docs...");
                if lib::run_cargo_doc(&cargo_doc_args).await.success() {
                    build_id.fetch_add(1, Ordering::Relaxed);
                    log::info!("Docs updated (build #{})", build_id.load(Ordering::Relaxed));
                }
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
            if !lib::run_cargo_doc(&self.cargo_doc_args()).await.success() {
                return Err(anyhow::anyhow!("failed to run cargo doc"));
            }
            let build_id = Arc::new(AtomicU64::new(0));
            self.watch(Arc::clone(&build_id))?;
            self.open()?;
            log::info!("Serving {content} on {url}");
            if self.watch {
                lib::serve_crate_doc_watch(&self.manifest_path(), &self.addr(), build_id).await?
            } else {
                lib::serve_crate_doc(&self.manifest_path(), &self.addr()).await?
            }
        })
    }
}
