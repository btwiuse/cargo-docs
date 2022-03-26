    #[macro_use]
    extern crate tower_web;

    use bytes::{Bytes, BytesMut};
    use futures::{Async, Future, Stream as _Stream};
    use replacing_buf_stream::{FreezingBufStream, ReplacingBufStream};
    use std::path::PathBuf;
    use tokio::fs::File;
    use tower_web::response::{Context, Serializer};
    use tower_web::ServiceBuilder;
    #[derive(Clone, Debug)]
    pub struct Server {
        crate_dir: PathBuf,
        rustup_dir: Option<PathBuf>,
    }

    impl Server {
        fn serve(
            &self,
            relative_path: PathBuf,
        ) -> impl Future<Item = FileOrRedirect, Error = std::io::Error> {
            let mut path = if relative_path.starts_with("crate") {
                self.crate_dir
                    .join(relative_path.strip_prefix("crate").unwrap())
            } else if relative_path.starts_with("rust") {
                let relative_path = relative_path.strip_prefix("rust").unwrap();
                if let Some(rustup_dir) = &self.rustup_dir {
                    rustup_dir.join(relative_path)
                } else {
                    return Box::new(futures::future::ok(FileOrRedirect::Redirect(format!(
                        "https://doc.rust-lang.org/nightly{}",
                        relative_path.display()
                    )))) as Box<Future<Item = _, Error = _> + Send>;
                }
            } else {
                return Box::new(futures::future::ok(FileOrRedirect::Bytes((
                    Bytes::from("Not found"),
                    http::StatusCode::NOT_FOUND,
                )))) as Box<Future<Item = _, Error = _> + Send>;
            };

            let f = tokio::fs::File::open(path.clone())
                .then(move |result| match result {
                    Ok(file) => {
                        let f = file.metadata().map(move |(file, metadata)| {
                            if metadata.file_type().is_dir() {
                                FileOrRedirect::Redirect(format!(
                                    "/{}",
                                    relative_path.join("index.html").display()
                                ))
                            } else {
                                FileOrRedirect::File(ReplacingBufStream::new(
                                    FreezingBufStream(file),
                                    Bytes::from("https://doc.rust-lang.org/nightly"),
                                    Bytes::from("/rust"),
                                ))
                            }
                        });
                        Box::new(f) as Box<Future<Item = _, Error = _> + Send>
                    }
                    Err(err) => {
                        if err.kind() == std::io::ErrorKind::NotFound
                            && path.ends_with("index.html")
                        {
                            path.pop();
                            let f = tokio::fs::read_dir(path.clone())
                                .and_then(|dir| dir.map(|entry| entry.path()).collect())
                                .map(move |children| {
                                    let mut paths: Vec<_> = children
                                        .into_iter()
                                        .map(|child| {
                                            format!(
                                                "{}",
                                                child.strip_prefix(&path).unwrap().display()
                                            )
                                        })
                                        .collect();
                                    paths.sort();
                                    let mut page = String::new();
                                    page.push_str("<!DOCTYPE html><html><body>");
                                    for path in paths {
                                        page.push_str(r#"<a href=""#);
                                        page.push_str(&path);
                                        page.push_str(r#"">"#);
                                        page.push_str(&path);
                                        page.push_str("</a><br />");
                                    }
                                    page.push_str("</body></html>");
                                    FileOrRedirect::Bytes((
                                        BytesMut::from(page).freeze(),
                                        http::StatusCode::OK,
                                    ))
                                });
                            Box::new(f) as Box<Future<Item = _, Error = _> + Send>
                        } else {
                            Box::new(futures::future::err(err))
                                as Box<Future<Item = _, Error = _> + Send>
                        }
                    }
                })
                .or_else(|err: std::io::Error| {
                    if err.kind() == std::io::ErrorKind::NotFound {
                        Ok(FileOrRedirect::Bytes((
                            Bytes::from("Not found"),
                            http::StatusCode::NOT_FOUND,
                        )))
                    } else {
                        Err(err)
                    }
                });
            Box::new(f) as Box<Future<Item = _, Error = _> + Send>
        }
    }

    impl_web! {
        impl Server {
            #[get("/")]
            fn root(&self) -> impl Future<Item=FileOrRedirect, Error=std::io::Error> {
                futures::future::ok(FileOrRedirect::Redirect("/index.html".to_owned()))
            }

            #[get("/index.html")]
            fn index(&self) -> impl Future<Item=&'static str, Error=std::io::Error> {
                futures::future::ok(r#"<!DOCTYPE html><html><body><a href="/crate">Crate-specific documentation</a><br /><a href="/rust">Rust book and std documentation</a></body></html>"#)
            }

            #[get("/*relative_path")]
            fn files(&self, relative_path: PathBuf) -> impl Future<Item=FileOrRedirect, Error=std::io::Error> {
                self.serve(relative_path)
            }
        }
    }

    type Stream = ReplacingBufStream<FreezingBufStream<File>>;

    enum FileOrRedirect {
        File(Stream),
        Redirect(String),
        Bytes((Bytes, http::StatusCode)),
    }

    enum StreamOrBytes {
        Stream(Stream),
        Bytes(Option<Bytes>),
    }

    impl tower_web::util::BufStream for StreamOrBytes {
        type Item = std::io::Cursor<Bytes>;
        type Error = std::io::Error;

        fn poll(&mut self) -> Result<Async<Option<Self::Item>>, Self::Error> {
            match self {
                StreamOrBytes::Stream(stream) => stream.poll(),
                StreamOrBytes::Bytes(bytes) => {
                    if let Some(bytes) = bytes.take() {
                        Ok(Async::Ready(Some(std::io::Cursor::new(bytes))))
                    } else {
                        Ok(Async::Ready(None))
                    }
                }
            }
        }
    }

    impl tower_web::response::Response for FileOrRedirect {
        type Buf = std::io::Cursor<Bytes>;
        type Body = tower_web::error::Map<StreamOrBytes>;

        fn into_http<S: Serializer>(
            self,
            context: &Context<S>,
        ) -> Result<http::Response<Self::Body>, tower_web::Error> {
            match self {
                FileOrRedirect::File(bufstream) => {
                    let content_type = context
                        .content_type_header()
                        .map(|header| header.clone())
                        .unwrap_or_else(|| {
                            http::header::HeaderValue::from_static("application/octet-stream")
                        });

                    Ok(http::Response::builder()
                        .status(http::StatusCode::OK)
                        .header(http::header::CONTENT_TYPE, content_type)
                        .body(tower_web::error::Map::new(StreamOrBytes::Stream(bufstream)))
                        .unwrap())
                }
                FileOrRedirect::Bytes((bytes, status_code)) => Ok(http::Response::builder()
                    .status(status_code)
                    .body(tower_web::error::Map::new(StreamOrBytes::Bytes(Some(
                        bytes,
                    ))))
                    .unwrap()),
                FileOrRedirect::Redirect(path) => Ok(http::Response::builder()
                    .status(http::StatusCode::FOUND)
                    .header("Location", path.as_str())
                    .body(tower_web::error::Map::new(StreamOrBytes::Bytes(None)))
                    .unwrap()),
            }
        }
    }
