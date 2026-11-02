use game_application::GameService;
use rpc::{GameRpc, proto::game_service_server::GameServiceServer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("GAME_SERVER_ADDR").ok())
        .unwrap_or_else(|| "127.0.0.1:50051".to_owned())
        .parse()?;
    eprintln!("game-server listening on {address}");
    tonic::transport::Server::builder()
        .add_service(GameServiceServer::new(GameRpc::new(GameService::new())))
        .serve_with_shutdown(address, async {
            if let Err(error) = tokio::signal::ctrl_c().await {
                eprintln!("could not listen for Ctrl-C: {error}");
            }
        })
        .await?;
    Ok(())
}
