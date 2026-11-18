use realtime_gateway::{Gateway, GatewayConfig};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = GatewayConfig::from_env()?;
    let bind = config.bind;
    let gateway = Gateway::new(config)?;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    eprintln!(
        "realtime-gateway-server listening on {}",
        listener.local_addr()?
    );
    let closing = gateway.clone();
    axum::serve(listener, gateway.router())
        .with_graceful_shutdown(async move {
            if let Err(error) = tokio::signal::ctrl_c().await {
                eprintln!("could not listen for Ctrl-C: {error}");
            }
            closing.shutdown();
        })
        .await?;
    if !gateway.wait_closed(Duration::from_secs(10)).await {
        return Err("gateway connections did not close before deadline".into());
    }
    Ok(())
}
