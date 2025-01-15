pub use axum_websockify::error::WebsockifyError;

use axum::{response::IntoResponse, routing::get, Router};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use axum::http::Uri;
use axum::response::Response;

use rust_embed::RustEmbed;
use std::env;
use tower_http::trace::TraceLayer;

use axum::http::{header, StatusCode};

#[derive(RustEmbed)]
#[folder = "noVNC"]
struct Asset;

pub struct StaticFile<T>(pub T);

impl<T> IntoResponse for StaticFile<T>
where
    T: Into<String>,
{
    fn into_response(self) -> Response {
        let path = self.0.into();

        match Asset::get(path.as_str()) {
            Some(content) => {
                let mime = mime_guess::from_path(path).first_or_octet_stream();
                ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
            }
            None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
        }
    }
}

async fn index_handler() -> impl IntoResponse {
    static_handler("/index.html".parse::<Uri>().unwrap()).await
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();

    if path.starts_with("static/") {
        path = path.replace("static/", "");
    }
    tracing::debug!("path requested : {}", path);
    StaticFile(path)
}

use clap::Parser;
#[derive(Parser, Debug)]
#[command(name = "WebSockify-rs")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(author = env!("CARGO_PKG_AUTHORS"))]
#[command(about = "Convert TCP/Unix domain socket connection to WebSocket")]
struct Cli {
    /// Upstream host:port
    #[arg(index = 1)]
    upstream: String,

    /// Listen host:port
    #[arg(index = 2)]
    listen: String,

    /// Server prefix
    #[arg(short, long)]
    prefix: Option<String>,

    /// Verbosity (can be used multiple times)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    for file in Asset::iter() {
        println!("{}", file.as_ref());
    }

    let args = Cli::parse();
    let verbosity = args.verbose;

    match verbosity {
        1 => env::set_var("RUST_LOG", "info"),
        2 => env::set_var("RUST_LOG", "debug"),
        3 => env::set_var("RUST_LOG", "trace"),
        _ => {
            if env::var("RUST_LOG").is_err() {
                env::set_var("RUST_LOG", "warn")
            }
        }
    }

    let upstream = axum_websockify::Destination::tcp(args.upstream).unwrap();

    let static_url = "/static/vnc.html".parse::<Uri>().unwrap();

    // Create the router
    let app = Router::new()
        .nest("/websockify", axum_websockify::create_router(upstream))
        .route("/static/*file", get(static_handler))
        .route("/index.html", get(index_handler))
        .route("/", get(index_handler))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&args.listen).await.unwrap();
    tracing::info!("URL: http://{}{}", args.listen, static_url);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
    Ok(())
}
