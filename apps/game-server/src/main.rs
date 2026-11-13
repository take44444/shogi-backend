use game_application::GameService;
use game_grpc::{GameRpc, proto::game_service_server::GameServiceServer};
use game_persistence::SeaOrmStore;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("GAME_SERVER_ADDR").ok())
        .unwrap_or_else(|| "127.0.0.1:50051".to_owned())
        .parse()?;
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://shogi:shogi@localhost:5432/shogi".to_owned());
    let store = Arc::new(SeaOrmStore::connect(&url).await.map_err(|error| {
        std::io::Error::other(format!("Game DB startup failed: {error}. This release requires schema version 6. For an older schema, preserve its data and provision a separate empty database."))
    })?);
    let service = GameRpc::new(GameService::new(store));
    eprintln!("game-server listening on {address}");
    tonic::transport::Server::builder()
        .add_service(GameServiceServer::new(service))
        .serve_with_shutdown(address, async {
            if let Err(error) = tokio::signal::ctrl_c().await {
                eprintln!("could not listen for Ctrl-C: {error}");
            }
        })
        .await?;
    Ok(())
}
