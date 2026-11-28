mod provider;
#[cfg(test)]
mod tests;

use std::{env, net::SocketAddr, sync::Arc};
use user_http::BrowserConfig;
use user_persistence::PostgresAccounts;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let origin = env::var("VERIFY_PUBLIC_ORIGIN")?;
    let browser = BrowserConfig::new(&origin, true)?;
    let url = url::Url::parse(&origin)?;
    if url.scheme() != "http"
        || !matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
    {
        return Err("verification requires a loopback HTTP public origin".into());
    }
    let database = env::var("VERIFY_DATABASE_URL")?;
    if !(database.starts_with("postgres://") || database.starts_with("postgresql://")) {
        return Err("VERIFY_DATABASE_URL must be PostgreSQL".into());
    }
    let bind: SocketAddr = env::var("VERIFY_BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".into())
        .parse()?;
    let store = Arc::new(PostgresAccounts::connect(&database).await?);
    let provider = Arc::new(provider::VerificationProvider::new(
        url.origin().ascii_serialization(),
    ));
    let app = user_http::router(store.clone(), provider.clone(), browser)
        .merge(provider::router(provider));
    let listener = tokio::net::TcpListener::bind(bind).await?;
    eprintln!(
        "verification user server listening on {}",
        listener.local_addr()?
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    store.close().await?;
    Ok(())
}
