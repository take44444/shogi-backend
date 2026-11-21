#![cfg(test)]

use async_trait::async_trait;
use game_application::{
    Acceptance, Action, ApplicationError as Error, GameId, GameRecord, GameRepository, MakeMove,
    MutationRequest, Participants, Receipt, RequestId, Resign, StagedGameChange, UserId,
};
use game_persistence::SeaOrmStore;
use pg_test_support::TestDatabase;
use sea_orm::{ConnectionTrait, Database};
use shogi_domain::Player;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, Ordering},
};

mod support;
use support::{FaultStore, id, integer_query, movement, participants, resign, setup, user};

mod availability;
mod coherent_reads;
mod concurrent_commands;
mod metadata_constraints;
mod participant_binding;
mod restoration;
mod schema;
mod transaction_failures;
