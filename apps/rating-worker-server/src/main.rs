use game_application::{
    GameId,
    rating::{RatingWorker, WorkerId, WorkerPolicy},
};
use game_persistence::SeaOrmRatingJobs;
use std::{num::NonZeroU32, sync::Arc, time::Duration};

fn number(name: &str, default: u64) -> Result<u64, Box<dyn std::error::Error>> {
    Ok(match std::env::var(name) {
        Ok(value) => value.parse()?,
        Err(std::env::VarError::NotPresent) => default,
        Err(e) => return Err(e.into()),
    })
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://shogi:shogi@localhost:5432/shogi".into());
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if !(args.is_empty() || args.len() == 2 && args[0] == "retry-quarantined") {
        return Err("usage: rating-worker-server [retry-quarantined GAME_ID]".into());
    }
    let limit = u32::try_from(number("RATING_RETRY_LIMIT", 5)?)?;
    let policy = WorkerPolicy::new(
        Duration::from_millis(number("RATING_LEASE_MS", 30000)?),
        NonZeroU32::new(limit).ok_or("RATING_RETRY_LIMIT must be positive")?,
        Duration::from_millis(number("RATING_BACKOFF_MS", 1000)?),
    )?;
    let owner = WorkerId::new(
        std::env::var("RATING_WORKER_ID")
            .unwrap_or_else(|_| format!("worker-{}", std::process::id())),
    )?;
    let poll = Duration::from_millis(number("RATING_POLL_MS", 250)?);
    if poll.is_zero() {
        return Err("RATING_POLL_MS must be positive".into());
    }
    let repository = Arc::new(SeaOrmRatingJobs::connect(&url).await.map_err(|error| {
        std::io::Error::other(format!("Game DB startup failed: {error}. This release requires schema version 7. For an older schema, preserve its data and provision a separate empty database."))
    })?);
    let worker = RatingWorker::new(repository.clone(), policy);
    if !args.is_empty() {
        println!(
            "{:?}",
            worker
                .retry_quarantined(&GameId::new(args[1].clone())?)
                .await?
        );
    } else {
        println!("rating-worker-server running as {}", owner.as_str());
        loop {
            tokio::select! {
                _=tokio::signal::ctrl_c()=>break,
                result=worker.tick(&owner)=> match result {
                    Ok(game_application::rating::TickOutcome::Idle)=>{},
                    Ok(outcome)=>println!("rating tick {outcome:?}"),
                    Err(error)=>eprintln!("rating tick failed: {error}"),
                }
            }
            tokio::select! {_=tokio::signal::ctrl_c()=>break,_=tokio::time::sleep(poll)=>{}}
        }
    }
    repository.close().await?;
    Ok(())
}
