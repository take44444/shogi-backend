#![cfg(test)]

use game_application::{
    GameId as WriteId, GameRepository, MakeMove, Participants, RequestId, Resign, UserId,
};
use game_persistence::SeaOrmStore;
use game_record_application::{
    Error, Filters, FinishReason, GameId, MAX_HISTORY, PublicAction, RecordRepository, Rule, Search,
};
use game_record_persistence::PostgresGameRecords;
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, TransactionTrait,
};
use shogi_domain::Player;
use std::sync::Arc;

mod support;
use support::{Fixture, current_rows, filters, ids, search};

mod keysets;
mod metadata;
mod reader_boundary;
mod restoration;
mod schema;
mod snapshot_history;
