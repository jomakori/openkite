//! Process entry point for the browser shell.

use openkite_web::serve;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(
                // `dioxus_document` logs an error whenever a component registers a stylesheet,
                // because the SSR renderer has no browser document to put it in. The markup is
                // emitted anyway; silence that one module instead of logging per request.
                |_| tracing_subscriber::EnvFilter::new("info,dioxus_document=off"),
            ),
        )
        .init();

    serve().await
}
