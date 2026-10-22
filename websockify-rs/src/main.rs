pub use axum_websockify::error::WebsockifyError;

use axum::{
    response::{IntoResponse, Redirect},
    routing::get,
    Router,
};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use axum::extract::State;
use axum::http::Uri;
use axum::response::Response;

use rust_embed::{EmbeddedFile, RustEmbed};
use std::env;
use tower_http::trace::TraceLayer;

use axum::http::{header, StatusCode};

#[derive(RustEmbed)]
#[folder = "novnc/1.2.0/noVNC"]
struct Asset12;

#[derive(RustEmbed)]
#[folder = "novnc/1.3.0/noVNC"]
struct Asset13;

#[derive(RustEmbed)]
#[folder = "novnc/1.6.0/noVNC"]
struct Asset16;

#[derive(RustEmbed)]
#[folder = "novnc/1.7.0/noVNC"]
struct Asset17;

/// noVNC release whose embedded frontend is served under `/static/`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum NoVncVersion {
    #[value(name = "1.2.0")]
    V1_2_0,
    #[value(name = "1.3.0")]
    V1_3_0,
    #[value(name = "1.6.0")]
    V1_6_0,
    #[value(name = "1.7.0")]
    V1_7_0,
}

fn embedded_file(version: NoVncVersion, path: &str) -> Option<EmbeddedFile> {
    match version {
        NoVncVersion::V1_2_0 => Asset12::get(path),
        NoVncVersion::V1_3_0 => Asset13::get(path),
        NoVncVersion::V1_6_0 => Asset16::get(path),
        NoVncVersion::V1_7_0 => Asset17::get(path),
    }
}

pub struct StaticFile {
    version: NoVncVersion,
    path: String,
}

impl IntoResponse for StaticFile {
    fn into_response(self) -> Response {
        match embedded_file(self.version, &self.path) {
            Some(content) => {
                let mime = mime_guess::from_path(&self.path).first_or_octet_stream();
                ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
            }
            None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
        }
    }
}

async fn static_handler(State(version): State<NoVncVersion>, uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();

    if path.starts_with("static/") {
        path = path.replace("static/", "");
    }
    tracing::debug!("path requested : {} (noVNC {:?})", path, version);
    StaticFile { version, path }
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

    /// Embedded noVNC version to serve
    #[arg(long, value_enum, default_value = "1.7.0")]
    novnc: NoVncVersion,

    /// Verbosity (can be used multiple times)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

fn create_app(upstream: axum_websockify::Destination, novnc: NoVncVersion) -> Router {
    Router::<NoVncVersion>::new()
        .route("/static/{*wildcard}", get(static_handler))
        .route(
            "/index.html",
            get(|| async { Redirect::permanent("/static/vnc.html") }),
        )
        .route(
            "/",
            get(|| async { Redirect::permanent("/static/vnc.html") }),
        )
        .with_state(novnc)
        .nest("/websockify", axum_websockify::create_router(upstream))
        .layer(TraceLayer::new_for_http())
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
    let app = create_app(upstream, args.novnc);

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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).expect("CLI should parse")
    }

    #[test]
    fn defaults_to_latest_novnc_version() {
        let args = parse(&["websockify-rs", "localhost:5901", "127.0.0.1:8080"]);
        assert_eq!(args.novnc, NoVncVersion::V1_7_0);
    }

    #[test]
    fn novnc_flag_selects_embedded_version() {
        let args = parse(&[
            "websockify-rs",
            "--novnc",
            "1.6.0",
            "localhost:5901",
            "127.0.0.1:8080",
        ]);
        assert_eq!(args.novnc, NoVncVersion::V1_6_0);
    }

    #[test]
    fn every_available_novnc_version_parses() {
        for (value, expected) in [
            ("1.2.0", NoVncVersion::V1_2_0),
            ("1.3.0", NoVncVersion::V1_3_0),
            ("1.6.0", NoVncVersion::V1_6_0),
            ("1.7.0", NoVncVersion::V1_7_0),
        ] {
            let args = parse(&[
                "websockify-rs",
                "--novnc",
                value,
                "localhost:5901",
                "127.0.0.1:8080",
            ]);
            assert_eq!(args.novnc, expected, "--novnc {value} should parse");
        }
    }

    #[test]
    fn unavailable_novnc_version_is_rejected() {
        let result = Cli::try_parse_from([
            "websockify-rs",
            "--novnc",
            "1.4.0",
            "localhost:5901",
            "127.0.0.1:8080",
        ]);
        assert!(
            result.is_err(),
            "1.4.0 is not embedded and must be rejected"
        );
    }

    #[test]
    fn embedded_assets_are_selected_per_version() {
        let versions = [
            NoVncVersion::V1_2_0,
            NoVncVersion::V1_3_0,
            NoVncVersion::V1_6_0,
            NoVncVersion::V1_7_0,
        ];
        let contents: std::collections::HashSet<_> = versions
            .iter()
            .map(|version| {
                embedded_file(*version, "vnc_lite.html")
                    .expect("vnc_lite.html should be embedded")
                    .data
                    .into_owned()
            })
            .collect();

        assert_eq!(contents.len(), 4, "each version must serve its own asset");
    }

    #[test]
    fn missing_asset_is_not_found_for_every_version() {
        for version in [
            NoVncVersion::V1_2_0,
            NoVncVersion::V1_3_0,
            NoVncVersion::V1_6_0,
            NoVncVersion::V1_7_0,
        ] {
            assert!(embedded_file(version, "this-file-does-not-exist").is_none());
        }
    }

    use axum::body::{to_bytes, Body};
    use axum::http::{header, Request, StatusCode};
    use tower::ServiceExt;

    async fn request(app: Router, uri: &str) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec();
        (status, headers, body)
    }

    #[tokio::test]
    async fn serves_asset_matching_selected_version_over_http() {
        for (version, directory) in [
            (NoVncVersion::V1_2_0, "1.2.0"),
            (NoVncVersion::V1_3_0, "1.3.0"),
            (NoVncVersion::V1_6_0, "1.6.0"),
            (NoVncVersion::V1_7_0, "1.7.0"),
        ] {
            let app = create_app(axum_websockify::Destination::Tcp(Vec::new()), version);
            let (status, headers, body) = request(app, "/static/vnc_lite.html").await;

            assert_eq!(status, StatusCode::OK, "noVNC {directory}");
            assert_eq!(
                headers.get(header::CONTENT_TYPE).unwrap(),
                "text/html",
                "noVNC {directory}"
            );
            let expected = std::fs::read(format!("novnc/{directory}/noVNC/vnc_lite.html"))
                .expect("reference asset should exist on disk");
            assert_eq!(body, expected, "served asset must match noVNC {directory}");
        }
    }

    #[tokio::test]
    async fn missing_static_file_returns_404() {
        let app = create_app(
            axum_websockify::Destination::Tcp(Vec::new()),
            NoVncVersion::V1_7_0,
        );
        let (status, _, _) = request(app, "/static/does-not-exist.js").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn root_redirects_to_vnc_html() {
        let app = create_app(
            axum_websockify::Destination::Tcp(Vec::new()),
            NoVncVersion::V1_6_0,
        );
        let (status, headers, _) = request(app, "/").await;
        assert_eq!(status, StatusCode::PERMANENT_REDIRECT);
        assert_eq!(headers.get(header::LOCATION).unwrap(), "/static/vnc.html");
    }
}
