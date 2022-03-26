use cargo::core::compiler::CompileMode;
use cargo::core::Workspace;
use cargo::ops::{compile, doc, CompileOptions, DocOptions};
use cargo::util::config::Config;
use clap::{crate_version, App, Arg};
// use poem::{Endpoint, handler, get, endpoint::StaticFilesEndpoint, listener::TcpListener, Route, Server, web::Redirect};
use std::path::{Path, PathBuf};

use futures_util::future;
use http::response::Builder as ResponseBuilder;
use http::{header, StatusCode};
use hyper::server::Server;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response};
use hyper_staticfile::Static;
use std::net::SocketAddr;

async fn handle_request<B>(
    req: Request<B>,
    static_: Static,
    crate_name: String,
) -> Result<Response<Body>, std::io::Error> {
    match req.uri().path() {
        "/" => Ok(ResponseBuilder::new()
            .status(StatusCode::MOVED_PERMANENTLY)
            .header(header::LOCATION, format!("/{}/", crate_name))
            .body(Body::empty())
            .expect("unable to build response")),
        _ => static_.clone().serve(req).await,
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let matches = App::new("cargo-serve-doc")
        .version(crate_version!())
        .arg(
            Arg::with_name("dummy")
                .hidden(true)
                .possible_value("serve-doc"),
        )
        .arg(
            Arg::with_name("port")
                .long("port")
                .takes_value(true)
                .default_value("8080"),
        )
        .arg(
            Arg::with_name("manifest-path")
                .long("manifest-path")
                .takes_value(true)
                .default_value("Cargo.toml"),
        )
        .get_matches();

    let port = matches.value_of("port").unwrap();
    /*
    let addr = format!("127.0.0.1:{}", port).parse().expect("Invalid port");
    */
    let mut manifest_path = PathBuf::from(matches.value_of("manifest-path").unwrap());
    if !manifest_path.is_absolute() {
        manifest_path = std::env::current_dir().unwrap().join(manifest_path);
    }

    let config = Config::default().expect("Error making cargo config");
    let workspace = Workspace::new(&manifest_path, &config).expect("Error making workspace");

    let options = DocOptions {
        open_result: false,
        compile_opts: CompileOptions::new(&config, CompileMode::Doc { deps: true })
            .expect("Making CompileOptions"),
    };
    println!("Generating documentation for crate...");
    let compilation = compile(&workspace, &options.compile_opts)?;
    let root_crate_names = &compilation.root_crate_names;
    let crate_name = root_crate_names
        .get(0)
        .ok_or_else(|| anyhow::anyhow!("no crates with documentation"))?;
    // let crate_name = String::from(crate_name);
    let kind = options.compile_opts.build_config.single_requested_kind()?;
    let path = compilation.root_output[&kind]
        .with_file_name("doc")
        .join(&crate_name)
        .join("index.html");
    println!("{:?}", &root_crate_names);
    println!("{:?}", &crate_name);
    println!("{:?}", &kind);
    println!("{:?}", &path);
    // doc(&workspace, &options).expect("Running doc");
    // println!("{:?}", workspace);
    // println!("{:?}", options);

    let crate_dir = workspace.target_dir().join("doc").into_path_unlocked();

    let rustup_dir = find_rustdoc().and_then(|rustdoc| {
        Some(
            rustdoc
                .parent()?
                .parent()?
                .join("share")
                .join("doc")
                .join("rust")
                .join("html"),
        )
    });

    println!("Serving on http://127.0.0.1:{}", port);
    println!("crate_dir = {:?}", crate_dir);
    println!("rustup_dir = {:?}", rustup_dir);
    /*
    println!("Serving on http://127.0.0.1:{}", port);

    ServiceBuilder::new()
        .resource(Server {
            crate_dir,
            rustup_dir,
        })
        .run(&addr)
        .unwrap();
    */

    // let app = Route::new().nest("/", StaticFilesEndpoint::new("./target/doc/").index_file("./cargo_docs/index.html"));
    // let app = Route::new().at("/", get(redir)).nest("/", StaticFilesEndpoint::new("./target/doc/"));
    // let app = Route::new().nest("/", get(index));
    // Server::new(TcpListener::bind(format!("127.0.0.1:{port}"))) .run(app) .await?;
    let static_ = Static::new(Path::new("target/doc/"));
    let make_service = make_service_fn(|_| {
        let static_ = static_.clone();
        let crate_name = crate_name.clone();
        future::ok::<_, hyper::Error>(service_fn(move |req| handle_request(req, static_.clone(), crate_name.clone())))
    });
    Server::bind(&format!("127.0.0.1:{port}").parse()?)
        .serve(make_service)
        .await?;
    Ok(())
}

fn find_rustdoc() -> Option<PathBuf> {
    let output = std::process::Command::new("rustup")
        .arg("which")
        .arg("rustdoc")
        .output()
        .unwrap();
    if output.status.success() {
        Some(PathBuf::from(String::from_utf8(output.stdout).unwrap()))
    } else {
        None
    }
}
