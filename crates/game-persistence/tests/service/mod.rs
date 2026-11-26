#![cfg(test)]

use game_application::{
    ApplicationError as Error, GameId, MakeMove, Participants, RequestId, Resign, UserId,
};
use pg_test_support::TestDatabase;
use shogi_domain::{CommandError, FinishReason, Player};
use std::sync::Arc;
use tokio::{sync::Barrier, task::JoinSet};

mod support;
use support::{id, movement, participants, resignation, setup};

mod concurrent_commands;
mod creation;
mod replay;

fn assert_same_snapshot(
    mut actual: game_application::Snapshot,
    expected: &game_application::Snapshot,
) {
    actual.server_now_ms = expected.server_now_ms;
    assert_eq!(&actual, expected);
}
