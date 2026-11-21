#![cfg(test)]

use async_trait::async_trait;
use game_application::matchmaking::{MatchmakingRepository, RefusalReason, TicketId, TicketState};
use game_application::rating::{
    ApplyOutcome, ClaimedRatingJob, JobFailure, Pool, RatingChange, RatingJobRepository,
    RatingState, RatingWorker, RatingWorkerError, RecoveryOutcome, TickOutcome, WorkerId,
    WorkerPolicy,
};
use game_application::{
    ApplicationError, GameId, GameRepository, GameService, Participants, RequestId, Resign, UserId,
};
use game_persistence::{SeaOrmRatingJobs, SeaOrmStore};
use game_record_application::ratings::RatingRepository;
use game_record_persistence::PostgresGameRecords;
use pg_test_support::TestDatabase;
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, TransactionTrait,
};
use std::{
    num::NonZeroU32,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

mod support;
use support::{
    Fixture, Uncertain, change, game, owner, players, policy, user, wait_for_blocked_transactions,
};

mod atomic_application;
mod claim_fences;
mod participation;
mod quarantine_recovery;
mod reader_permissions;
mod terminal_jobs;
mod worker_reconciliation;
