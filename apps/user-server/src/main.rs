use std::sync::Arc;
use user_http::{GoogleProvider, ServerConfig};
use user_persistence::PostgresAccounts;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ServerConfig::from_env()?;
    let store = Arc::new(PostgresAccounts::connect(&config.database_url).await?);
    let provider = Arc::new(
        GoogleProvider::discover(
            config.google_client_id,
            config.google_client_secret,
            config.browser.callback_url(),
        )
        .await?,
    );
    let app = user_http::router(store.clone(), provider, config.browser);
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    eprintln!("user HTTP server listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    store.close().await?;
    Ok(())
}
