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

include!("trace/game-play/binding/ids_validate_bytes_and_unicode_whitespace.rs");

include!("trace/game-play/binding/concurrent_creation_has_exactly_one_winner_and_shared_state.rs");

include!(
    "trace/game-command-recovery/actor-apply/competing_commands_at_one_sequence_accept_exactly_one.rs"
);

include!(
    "trace/game-command-recovery/replay/concurrent_identical_retries_all_receive_original_acceptance.rs"
);

include!(
    "trace/game-command-recovery/replay/every_rejection_preserves_snapshot_and_does_not_reserve_id.rs"
);

include!(
    "trace/game-command-recovery/replay/replay_precedes_stale_and_finished_checks_and_fingerprint_is_exact.rs"
);

include!(
    "trace/game-command-recovery/replay/games_and_independent_services_have_separate_id_namespaces.rs"
);

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

#[cfg(test)]
#[path = "../../../scripts/traceability_model.rs"]
mod trace_support;
