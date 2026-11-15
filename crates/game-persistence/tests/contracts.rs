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

fn user(value: &str) -> UserId {
    UserId::new(value).unwrap()
}
fn id() -> GameId {
    GameId::new("game").unwrap()
}
fn participants() -> Participants {
    Participants::new(user("black"), user("white")).unwrap()
}
fn movement(request: &str, expected_seq: u64, user_id: &str, usi: &str) -> MakeMove {
    MakeMove {
        request_id: RequestId::new(request).unwrap(),
        expected_seq,
        user_id: user(user_id),
        movement: usi.parse().unwrap(),
    }
}
fn resign() -> Resign {
    Resign {
        request_id: RequestId::new("end").unwrap(),
        expected_seq: 5,
        user_id: user("black"),
    }
}
async fn setup(url: &str) -> (Arc<SeaOrmStore>, pg_test_support::GameFixture) {
    let store = Arc::new(SeaOrmStore::connect(url).await.unwrap());
    let service = pg_test_support::GameFixture::new(store.clone(), url);
    service.seed(id(), participants()).await.unwrap();
    (store, service)
}

include!(
    "trace/game-snapshots-access/snapshot-refresh/closing_the_pool_makes_the_game_service_unavailable.rs"
);

include!(
    "trace/game-play/binding/distinct_ids_create_games_without_accounts_and_bindings_are_immutable.rs"
);

include!(
    "trace/game-snapshots-access/restore/reconnecting_restores_position_hands_promotion_and_receipts_even_after_finish.rs"
);

include!(
    "trace/game-command-recovery/actor-apply/independent_instances_cas_refresh_and_identical_concurrent_replay.rs"
);

include!(
    "trace/game-command-recovery/durable-commit/real_statement_and_commit_failures_roll_back_both_metadata_and_receipt.rs"
);

include!(
    "trace/game-command-recovery/durable-commit/direct_store_constraints_and_sequence_overflow_leave_no_changes.rs"
);

include!("trace/game-snapshots-access/restore/corrupt_history_and_metadata_fail_closed.rs");

struct FaultStore {
    inner: Arc<SeaOrmStore>,
    mode: AtomicU8,
    unreadable: AtomicBool,
}
#[async_trait]
impl GameRepository for FaultStore {
    async fn load_game_for_viewer(
        &self,
        id: &GameId,
        viewer: &UserId,
    ) -> Result<game_application::ViewerGameRecord, Error> {
        self.inner.load_game_for_viewer(id, viewer).await
    }
    async fn view_metadata(
        &self,
        id: &GameId,
        viewer: &UserId,
    ) -> Result<game_application::ViewMetadata, Error> {
        self.inner.view_metadata(id, viewer).await
    }

    async fn load_game(&self, id: &GameId) -> Result<GameRecord, Error> {
        if self.unreadable.load(Ordering::SeqCst) {
            Err(Error::Unavailable)
        } else {
            self.inner.load_game(id).await
        }
    }
    async fn version(&self, id: &GameId) -> Result<u64, Error> {
        if self.unreadable.load(Ordering::SeqCst) {
            Err(Error::Unavailable)
        } else {
            self.inner.version(id).await
        }
    }
    async fn commit_change(&self, change: &StagedGameChange) -> Result<(), Error> {
        let mode = self.mode.swap(0, Ordering::SeqCst);
        if mode == 1 {
            return Err(Error::Unavailable);
        }
        if mode == 4 {
            let mut different = change.receipt().clone();
            different.request.user_id = user("white");
            different.request.action = Action::Resign;
            different.acceptance.result = Some(shogi_domain::GameResult {
                winner: Player::Black,
                reason: shogi_domain::FinishReason::Resignation,
            });
            let different = StagedGameChange::new(change.game_id().clone(), different)?;
            self.inner.commit_change(&different).await?;
        } else {
            self.inner.commit_change(change).await?;
        }
        if mode == 3 {
            self.unreadable.store(true, Ordering::SeqCst);
        }
        if mode >= 2 {
            Err(Error::Unavailable)
        } else {
            Ok(())
        }
    }
}

include!(
    "trace/game-command-recovery/actor-apply/uncertain_outcomes_reconcile_durable_receipts_or_fence_until_recovery.rs"
);

include!(
    "trace/startup-schema-isolation/contract-game-schema/schema_bootstrap_is_serialized_and_future_versions_are_rejected.rs"
);

include!(
    "trace/startup-schema-isolation/contract-game-schema/an_existing_empty_schema_ledger_is_rejected_without_modification.rs"
);

include!(
    "trace/startup-schema-isolation/contract-game-schema/a_missing_schema_ledger_is_not_recreated_for_existing_game_tables.rs"
);

include!(
    "trace/public-record-detail/metadata-invariants/postgres_enforces_result_pairs_foreign_keys_and_distinct_participants.rs"
);

include!(
    "trace/game-snapshots-access/durable-read/joined_history_load_remains_coherent_during_concurrent_commits.rs"
);

async fn integer_query(db: &sea_orm::DatabaseConnection, sql: &str) -> i64 {
    db.query_one_raw(sea_orm::Statement::from_string(
        sea_orm::DbBackend::Postgres,
        sql,
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "value")
    .unwrap()
}

include!(
    "trace/startup-schema-isolation/contract-game-schema/fresh_game_database_has_no_account_tables.rs"
);

include!(
    "trace/startup-schema-isolation/contract-game-schema/failed_bootstrap_rolls_back_the_entire_schema.rs"
);

include!(
    "trace/startup-schema-isolation/contract-game-schema/incompatible_or_incomplete_existing_schema_is_not_modified.rs"
);

include!(
    "trace/startup-schema-isolation/contract-game-schema/unrelated_tables_require_a_current_application_schema.rs"
);

include!(
    "trace/game-play/binding/identifiers_with_nul_and_backslashes_round_trip_without_collisions.rs"
);

#[cfg(test)]
#[path = "../../../scripts/traceability_model.rs"]
mod trace_support;
