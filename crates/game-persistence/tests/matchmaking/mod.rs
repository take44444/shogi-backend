#![cfg(test)]

use game_application::matchmaking::{
    HeartbeatSequence, MatchmakingError, MatchmakingRepository, Participation, RefusalReason,
    Ticket, TicketId, TicketState,
};
use game_application::rating::{Pool, RatingState};
use game_application::{GameId, GameRepository, GameService, RequestId, Resign, UserId};
use game_persistence::SeaOrmStore;
use pg_test_support::TestDatabase;
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, TransactionTrait,
};
use std::sync::Arc;

mod support;
use support::{Fixture, POOL, active, uid};

mod admission;
mod cancellation;
mod heartbeat;
mod pairing;
mod readiness;
mod terminal_rollback;
