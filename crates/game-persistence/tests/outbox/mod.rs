#![cfg(test)]

use async_trait::async_trait;
use game_application::{
    ApplicationError as Error, DomainEvent, GameId, GameRecord, GameRepository, MakeMove,
    OutboxRepository, Participants, RequestId, Resign, StagedGameChange, UserId,
};
use game_persistence::{PostgresOutbox, SeaOrmStore};
use pg_test_support::TestDatabase;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use std::sync::Arc;

mod support;
use support::{Uncertain, command, count, id, setup, user};

#[test]
fn publisher_lock_probe() {
    let Ok(url) = std::env::var("OUTBOX_LOCK_PROBE_URL") else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let queue = runtime.block_on(PostgresOutbox::connect(&url));
    let expected = std::env::var("OUTBOX_LOCK_PROBE_HELD").unwrap() == "1";
    assert_eq!(queue.is_err(), expected);
    if expected {
        assert!(matches!(
            queue,
            Err(game_persistence::OutboxOpenError::PublisherAlreadyRunning)
        ));
    }
}

mod atomic_dispatch;
mod exclusive_queue;
mod schema;
