#![cfg(test)]

use game_application::{
    ApplicationError as Error, GameId, MakeMove, Participants, RequestId, Resign, UserId,
};
use pg_test_support::TestDatabase;
use shogi_domain::{CommandError, FinishReason, Player};
use std::sync::Arc;
use tokio::{sync::Barrier, task::JoinSet};

fn id(value: &str) -> GameId {
    GameId::new(value).unwrap()
}
fn movement(request: &str, seq: u64, player: Player, usi: &str) -> MakeMove {
    MakeMove {
        request_id: RequestId::new(request).unwrap(),
        expected_seq: seq,
        user_id: user(player),
        movement: usi.parse().unwrap(),
    }
}
fn resignation(request: &str, seq: u64, player: Player) -> Resign {
    Resign {
        request_id: RequestId::new(request).unwrap(),
        expected_seq: seq,
        user_id: user(player),
    }
}

#[path = "binding/service.rs"]
mod game_play_binding;

#[path = "../game-command-recovery/actor-apply/service.rs"]
mod game_command_recovery_actor_apply;

#[path = "../game-command-recovery/replay/tests.rs"]
mod game_command_recovery_replay;

fn user(side: Player) -> UserId {
    UserId::new(match side {
        Player::Black => "black",
        Player::White => "white",
    })
    .unwrap()
}
fn participants() -> Participants {
    Participants::new(user(Player::Black), user(Player::White)).unwrap()
}
async fn setup() -> (TestDatabase, pg_test_support::GameFixture) {
    let database = TestDatabase::new().await;
    let store = Arc::new(
        game_persistence::SeaOrmStore::connect(database.url())
            .await
            .unwrap(),
    );
    {
        let games = pg_test_support::GameFixture::new(store, database.url());
        (database, games)
    }
}
