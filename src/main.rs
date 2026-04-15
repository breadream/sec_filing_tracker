mod company_facts;
mod diff;
mod error;
mod filing_fetcher;
mod filing_locator;
mod financial_metrics;
mod models;
mod routes;
mod sec_client;
mod section_parser;
mod summarizer;
mod trend_analyzer;
mod warning_signs;

use std::{net::SocketAddr, sync::Arc, time::Duration};

use sec_client::SecClient;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sec_filing_tracker=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let user_agent = std::env::var("SEC_USER_AGENT")
        .unwrap_or_else(|_| "sec-filing-change-tracker/0.1 contact@example.com".to_string());

    let client = Arc::new(SecClient::new(
        &user_agent,
        Duration::from_secs(20),
        Duration::from_millis(150),
    )?);

    let app = routes::router(client);
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("listening on http://{addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
