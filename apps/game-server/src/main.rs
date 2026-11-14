use game_application::{GameService, matchmaking::MatchmakingRepository, rating::Pool};
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
        std::io::Error::other(format!("Game DB startup failed: {error}. This release requires schema version 7. For an older schema, preserve its data and provision a separate empty database."))
    })?);
    let service = GameRpc::new(GameService::new(store.clone()), store.clone());
    let (stop, mut stopped) = tokio::sync::watch::channel(false);
    let matcher = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut first = 0;
        let mut failures = 0_u64;
        loop {
            tokio::select! {
                _ = stopped.changed() => break,
                _ = interval.tick() => {
                    let mut empty = 0;
                    for pair in 0..32 {
                        let pool = Pool::ALL[(first + pair) % 2];
                        match store.tick(pool).await {
                            Ok(Some(_)) => { empty = 0; failures = 0; }
                            Ok(None) => { empty += 1; failures = 0; if empty == 2 { break; } }
                            Err(error) => {
                                failures = failures.saturating_add(1);
                                eprintln!("matchmaking tick failed: pool={} consecutive_failures={failures} error={error}", pool.as_str());
                                break;
                            }
                        }
                    }
                    first = 1 - first;
                }
            }
        }
    });
    eprintln!("game-server listening on {address}");
    tonic::transport::Server::builder()
        .add_service(GameServiceServer::new(service))
        .serve_with_shutdown(address, async {
            if let Err(error) = tokio::signal::ctrl_c().await {
                eprintln!("could not listen for Ctrl-C: {error}");
            }
        })
        .await?;
    let _ = stop.send(true);
    matcher.await?;
    Ok(())
}
