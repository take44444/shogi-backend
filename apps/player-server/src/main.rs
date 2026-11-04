use game_persistence::PostgresGameRecords;
use game_record_application::GameRecords;
use std::{net::SocketAddr, sync::Arc};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("PLAYER_GAME_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://shogi_reader:shogi_reader@localhost:5432/shogi".into());
    let address: SocketAddr = std::env::var("PLAYER_BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8081".into())
        .parse()?;
    let repository = Arc::new(PostgresGameRecords::connect(&url).await?);
    let app = player_http::router(GameRecords::new(repository.clone()));
    let listener = tokio::net::TcpListener::bind(address).await?;
    println!("player-server listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    repository.close().await;
    Ok(())
}
