//! Browser shell: axum server that hosts the console.

use axum::{
    response::{Html, IntoResponse},
    routing::get,
    Router,
};

/// Version baked in at build time; the release workflow passes the git tag.
const BUILD_VERSION: &str = match option_env!("OPENKITE_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

/// Routes served by the browser shell.
pub fn router() -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/", get(index))
}

/// Serve until the process receives SIGTERM or Ctrl-C.
pub async fn serve() -> std::io::Result<()> {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8080);
    let address = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    let listener = tokio::net::TcpListener::bind(address).await?;
    tracing::info!(%address, version = version(), "openkite-web listening");

    axum::serve(listener, router())
        .with_graceful_shutdown(shutdown_signal())
        .await
}

/// Release tag from the environment, falling back to the build-time value.
fn version() -> String {
    std::env::var("OPENKITE_VERSION").unwrap_or_else(|_| BUILD_VERSION.to_owned())
}

async fn healthz() -> &'static str {
    "ok"
}

async fn index() -> impl IntoResponse {
    Html(index_html(&version()))
}

fn index_html(version: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>OpenKite</title>
<style>
  :root {{ color-scheme: dark; }}
  body {{
    margin: 0;
    min-height: 100vh;
    display: grid;
    place-items: center;
    background: #0b0d10;
    color: #e6e9ef;
    font: 16px/1.6 ui-sans-serif, system-ui, -apple-system, sans-serif;
  }}
  main {{ max-width: 34rem; padding: 2rem; }}
  h1 {{ margin: 0 0 .25rem; font-size: 1.5rem; letter-spacing: -.01em; }}
  p {{ margin: 0 0 1rem; color: #9aa4b2; }}
  code {{
    font: 13px/1.4 ui-monospace, SFMono-Regular, Menlo, monospace;
    background: #151a21;
    border: 1px solid #232a34;
    border-radius: 6px;
    padding: .15rem .4rem;
    color: #cbd5e1;
  }}
  footer {{ margin-top: 2rem; font-size: 13px; color: #6b7686; }}
</style>
</head>
<body>
<main>
  <h1>OpenKite</h1>
  <p>The browser shell is running, but the console arrives with the engine.
     This page is the v2 placeholder for release <code>{version}</code>.</p>
  <footer>Health check: <code>/healthz</code></footer>
</main>
</body>
</html>
"#
    )
}

async fn shutdown_signal() {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    let terminate = async {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            match signal(SignalKind::terminate()) {
                Ok(mut stream) => {
                    stream.recv().await;
                }
                Err(error) => tracing::error!(%error, "cannot watch SIGTERM"),
            }
        }
        #[cfg(not(unix))]
        std::future::pending::<()>().await;
    };

    tokio::select! {
        () = interrupt => {}
        () = terminate => {}
    }
    tracing::info!("shutdown signal received");
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn healthz_reports_ok() {
        let response = router()
            .oneshot(
                axum::http::Request::get("/healthz")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"ok");
    }

    #[tokio::test]
    async fn index_serves_html() {
        let response = router()
            .oneshot(
                axum::http::Request::get("/")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("OpenKite"));
    }
}
