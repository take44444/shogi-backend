//! Game use cases backed by [`GameRepository`].
//!
//! Matchmaking creates games with permanent side bindings. [`GameService`] derives
//! command sides from [`Participants`]. Account management and
//! authentication belong to a separate service.
//! The application contains no database types and has no default in-memory store.
//!
//! Each game service registry runs one actor per game with a 64-command mailbox.
//! Clones share that registry. Registry locks do not span database or channel waits.
//! Unknown games do not retain actors; closed actors reload durable state on demand.
//! Before each snapshot or mutation, the actor checks the durable version and
//! reloads changed or invalidated state. Failed reads return errors even when a
//! cached position exists. Dropping the last service closes its senders; actors
//! drain queued commands and exit.
//!
//! # Sequences and retries
//!
//! Games start at sequence zero. Each accepted move or resignation advances the
//! sequence once, up to `i64::MAX`. Only accepted mutations reserve request IDs,
//! scoped to a game. After checking participant membership, exact retries match
//! action kind, user, expected sequence, and typed move. They return the original
//! [`Acceptance`] before stale-sequence or finished-game checks, even after restart.
//! Rejected request IDs remain reusable. Matchmaking preserves durable ticket identities.
//!
//! A command stages a cloned game and receipt. The actor constructs a
//! [`StagedGameChange`] to generate and freeze events before persistence. The repository
//! atomically saves that supplied receipt and event batch. The actor publishes the candidate
//! only after [`GameRepository::commit_change`] confirms a commit. Every failed commit
//! triggers a durable reload. A matching receipt yields its original acceptance; changed
//! contents yield [`ApplicationError::RequestConflict`]. If commit reported a version
//! conflict and no matching receipt exists, the actor returns
//! [`ApplicationError::SequenceMismatch`] with the reloaded version. Other commit
//! failures without a receipt yield [`ApplicationError::Unavailable`] and invalidate
//! local state. Failed
//! reconciliation also leaves state invalid; a later successful refresh can recover.
//!
//! Cancellation after enqueue does not cancel the actor's transaction. A lost
//! response or `Unavailable` can mean the command committed. Retry identical
//! contents with the same request ID, including the original expected sequence.
#![warn(missing_docs)]

mod actor;
mod events;
pub use events::*;
pub mod matchmaking;
pub mod rating;
mod repository;
mod types;

use actor::{Actor, Command, snapshot_from_record};
pub use repository::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc, oneshot};
pub use types::*;

const MAILBOX_CAPACITY: usize = 64;

/// Cloneable application facade backed by durable storage and per-game actors.
///
/// See the [crate documentation](crate) for cancellation and retry contracts.
#[derive(Clone)]
pub struct GameService {
    repository: Arc<dyn GameRepository>,
    games: Arc<Mutex<HashMap<GameId, mpsc::Sender<Command>>>>,
}

impl GameService {
    /// Creates a service with an empty actor registry and the supplied game repository.
    /// No database access occurs until an operation is called.
    pub fn new(repository: Arc<dyn GameRepository>) -> Self {
        Self {
            repository,
            games: Arc::default(),
        }
    }

    /// Returns an owned snapshot after checking the durable version.
    /// Changed or invalid state is restored by validated journal replay.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::NotFound`] for an unknown game,
    /// [`ApplicationError::Corrupt`] for invalid history, or
    /// [`ApplicationError::Unavailable`] for storage or actor failure.
    ///
    /// # Panics
    ///
    /// Panics if actor creation is needed outside a Tokio runtime.
    pub async fn snapshot(&self, id: &GameId) -> Result<Snapshot, ApplicationError> {
        let sender = self.sender(id).await?;
        let (reply, response) = oneshot::channel();
        sender
            .send(Command::Snapshot(reply))
            .await
            .map_err(|_| ApplicationError::Unavailable)?;
        response.await.map_err(|_| ApplicationError::Unavailable)?
    }

    /// Returns a snapshot from one coherent visibility and history read.
    /// Inaccessible games have the same result as absent games.
    pub async fn snapshot_for_viewer(
        &self,
        id: &GameId,
        viewer: &UserId,
    ) -> Result<Snapshot, ApplicationError> {
        let view = self.repository.load_game_for_viewer(id, viewer).await?;
        if !view.access.permits(viewer) {
            return Err(ApplicationError::NotFound);
        }
        if view.record.id != *id {
            return Err(ApplicationError::Corrupt);
        }
        snapshot_from_record(view.record)
    }

    /// Checks current visibility without loading the game's history.
    pub async fn check_view_access(
        &self,
        id: &GameId,
        viewer: &UserId,
    ) -> Result<(), ApplicationError> {
        let access = self.repository.view_metadata(id, viewer).await?;
        if access.permits(viewer) {
            Ok(())
        } else {
            Err(ApplicationError::NotFound)
        }
    }

    /// Applies a participant's move, or replays an identical accepted request.
    /// The binding determines the player. Cancellation after enqueue does not stop execution.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::NonParticipant`] before receipt lookup for an unbound user.
    /// A changed accepted request returns [`ApplicationError::RequestConflict`]; a new
    /// request with a stale sequence returns [`ApplicationError::SequenceMismatch`].
    /// Also returns domain rejection, sequence exhaustion, lookup, restoration, or
    /// storage errors. [`ApplicationError::Unavailable`] can mean the command committed;
    /// retry identical contents with the same request ID.
    ///
    /// # Panics
    ///
    /// Panics if actor creation is needed outside a Tokio runtime.
    pub async fn make_move(
        &self,
        id: &GameId,
        command: MakeMove,
    ) -> Result<Acceptance, ApplicationError> {
        self.mutate(id, command.into()).await
    }

    /// Resigns on behalf of a bound participant, including off turn.
    /// Exact retries return the original acceptance, including after game completion.
    ///
    /// # Errors
    ///
    /// Returns the membership, replay, sequence, storage, and restoration errors
    /// described by [`Self::make_move`]. A new resignation after completion returns
    /// [`ApplicationError::Domain`] containing [`shogi_domain::CommandError::GameFinished`].
    ///
    /// # Panics
    ///
    /// Panics if actor creation is needed outside a Tokio runtime.
    pub async fn resign(
        &self,
        id: &GameId,
        command: Resign,
    ) -> Result<Acceptance, ApplicationError> {
        self.mutate(id, command.into()).await
    }

    async fn sender(&self, id: &GameId) -> Result<mpsc::Sender<Command>, ApplicationError> {
        {
            let games = self
                .games
                .lock()
                .map_err(|_| ApplicationError::Unavailable)?;
            if let Some(sender) = games.get(id).filter(|sender| !sender.is_closed()) {
                return Ok(sender.clone());
            }
        }
        self.repository.version(id).await?;
        let mut games = self
            .games
            .lock()
            .map_err(|_| ApplicationError::Unavailable)?;
        if let Some(sender) = games.get(id).filter(|sender| !sender.is_closed()) {
            return Ok(sender.clone());
        }
        let (sender, receiver) = mpsc::channel(MAILBOX_CAPACITY);
        let actor = Actor::new(id.clone(), self.repository.clone());
        games.insert(id.clone(), sender.clone());
        tokio::spawn(actor.run(receiver));
        Ok(sender)
    }

    async fn mutate(
        &self,
        id: &GameId,
        receipt: MutationRequest,
    ) -> Result<Acceptance, ApplicationError> {
        let sender = self.sender(id).await?;
        let (reply, response) = oneshot::channel();
        sender
            .send(Command::Mutate { receipt, reply })
            .await
            .map_err(|_| ApplicationError::Unavailable)?;
        response.await.map_err(|_| ApplicationError::Unavailable)?
    }
}

#[cfg(test)]
#[path = "trace/durable-event-dispatch/canonical-facts/contract.rs"]
mod trace_canonical_facts_contract;
#[cfg(test)]
#[path = "trace/durable-event-dispatch/confirmed-dispatch/contract.rs"]
mod trace_confirmed_dispatch_contract;
#[cfg(test)]
#[path = "trace/durable-event-dispatch/feature.rs"]
mod trace_feature;
#[cfg(test)]
#[path = "../../../scripts/traceability_model.rs"]
mod trace_support;

#[cfg(test)]
#[path = "trace/game-command-recovery/feature.rs"]
mod trace_feature_game_command_recovery;

#[cfg(test)]
#[path = "trace/game-command-recovery/replay/contract.rs"]
mod trace_contract_game_command_recovery_replay;

#[cfg(test)]
#[path = "trace/game-command-recovery/actor-apply/contract.rs"]
mod trace_contract_game_command_recovery_actor_apply;

#[cfg(test)]
#[path = "trace/game-command-recovery/sequence-cap/contract.rs"]
mod trace_contract_game_command_recovery_sequence_cap;

#[cfg(test)]
#[path = "trace/game-command-recovery/durable-commit/contract.rs"]
mod trace_contract_game_command_recovery_durable_commit;

#[cfg(test)]
#[path = "trace/game-command-recovery/canonical-events/contract.rs"]
mod trace_contract_game_command_recovery_canonical_events;

#[cfg(test)]
#[path = "trace/game-command-recovery/reconciliation/contract.rs"]
mod trace_contract_game_command_recovery_reconciliation;

#[cfg(test)]
#[path = "trace/game-command-recovery/mailbox/contract.rs"]
mod trace_contract_game_command_recovery_mailbox;

#[cfg(test)]
#[path = "trace/game-play/feature.rs"]
mod trace_feature_game_play;

#[cfg(test)]
#[path = "trace/game-play/notation/contract.rs"]
mod trace_contract_game_play_notation;

#[cfg(test)]
#[path = "trace/game-play/legal-moves/contract.rs"]
mod trace_contract_game_play_legal_moves;

#[cfg(test)]
#[path = "trace/game-play/domain-play/contract.rs"]
mod trace_contract_game_play_domain_play;

#[cfg(test)]
#[path = "trace/game-play/domain-resign/contract.rs"]
mod trace_contract_game_play_domain_resign;

#[cfg(test)]
#[path = "trace/game-play/binding/contract.rs"]
mod trace_contract_game_play_binding;

#[cfg(test)]
#[path = "trace/game-play/rpc-move/contract.rs"]
mod trace_contract_game_play_rpc_move;

#[cfg(test)]
#[path = "trace/game-play/rpc-resign/contract.rs"]
mod trace_contract_game_play_rpc_resign;

#[cfg(test)]
#[path = "trace/game-play/rpc-errors/contract.rs"]
mod trace_contract_game_play_rpc_errors;

#[cfg(test)]
#[path = "trace/game-snapshots-access/feature.rs"]
mod trace_feature_game_snapshots_access;

#[cfg(test)]
#[path = "trace/game-snapshots-access/snapshot-refresh/contract.rs"]
mod trace_contract_game_snapshots_access_snapshot_refresh;

#[cfg(test)]
#[path = "trace/game-snapshots-access/owned-position/contract.rs"]
mod trace_contract_game_snapshots_access_owned_position;

#[cfg(test)]
#[path = "trace/game-snapshots-access/grpc-shape/contract.rs"]
mod trace_contract_game_snapshots_access_grpc_shape;

#[cfg(test)]
#[path = "trace/game-snapshots-access/viewer-read/contract.rs"]
mod trace_contract_game_snapshots_access_viewer_read;

#[cfg(test)]
#[path = "trace/game-snapshots-access/access-policy/contract.rs"]
mod trace_contract_game_snapshots_access_access_policy;

#[cfg(test)]
#[path = "trace/game-snapshots-access/grpc-viewer/contract.rs"]
mod trace_contract_game_snapshots_access_grpc_viewer;

#[cfg(test)]
#[path = "trace/game-snapshots-access/restore/contract.rs"]
mod trace_contract_game_snapshots_access_restore;

#[cfg(test)]
#[path = "trace/game-snapshots-access/durable-read/contract.rs"]
mod trace_contract_game_snapshots_access_durable_read;

#[cfg(test)]
#[path = "trace/matchmaking-pairing/feature.rs"]
mod trace_feature_matchmaking_pairing;

#[cfg(test)]
#[path = "trace/matchmaking-pairing/windows/contract.rs"]
mod trace_contract_matchmaking_pairing_windows;

#[cfg(test)]
#[path = "trace/matchmaking-pairing/selection/contract.rs"]
mod trace_contract_matchmaking_pairing_selection;

#[cfg(test)]
#[path = "trace/matchmaking-pairing/durable-tick/contract.rs"]
mod trace_contract_matchmaking_pairing_durable_tick;

#[cfg(test)]
#[path = "trace/matchmaking-pairing/periodic/contract.rs"]
mod trace_contract_matchmaking_pairing_periodic;

#[cfg(test)]
#[path = "trace/matchmaking-tickets/feature.rs"]
mod trace_feature_matchmaking_tickets;

#[cfg(test)]
#[path = "trace/matchmaking-tickets/admission/contract.rs"]
mod trace_contract_matchmaking_tickets_admission;

#[cfg(test)]
#[path = "trace/matchmaking-tickets/reads/contract.rs"]
mod trace_contract_matchmaking_tickets_reads;

#[cfg(test)]
#[path = "trace/matchmaking-tickets/heartbeat/contract.rs"]
mod trace_contract_matchmaking_tickets_heartbeat;

#[cfg(test)]
#[path = "trace/matchmaking-tickets/cancel/contract.rs"]
mod trace_contract_matchmaking_tickets_cancel;

#[cfg(test)]
#[path = "trace/matchmaking-tickets/rpc-boundary/contract.rs"]
mod trace_contract_matchmaking_tickets_rpc_boundary;

#[cfg(test)]
#[path = "trace/matchmaking-tickets/browser-owner/contract.rs"]
mod trace_contract_matchmaking_tickets_browser_owner;

#[cfg(test)]
#[path = "trace/matchmaking-tickets/browser-response/contract.rs"]
mod trace_contract_matchmaking_tickets_browser_response;

#[cfg(test)]
#[path = "trace/matchmaking-tickets/browser-schema/contract.rs"]
mod trace_contract_matchmaking_tickets_browser_schema;

#[cfg(test)]
#[path = "trace/rating-application/feature.rs"]
mod trace_feature_rating_application;

#[cfg(test)]
#[path = "trace/rating-application/pure-calculation/contract.rs"]
mod trace_contract_rating_application_pure_calculation;

#[cfg(test)]
#[path = "trace/rating-application/terminal-job/contract.rs"]
mod trace_contract_rating_application_terminal_job;

#[cfg(test)]
#[path = "trace/rating-application/worker-reconciliation/contract.rs"]
mod trace_contract_rating_application_worker_reconciliation;

#[cfg(test)]
#[path = "trace/rating-application/claims-fences/contract.rs"]
mod trace_contract_rating_application_claims_fences;

#[cfg(test)]
#[path = "trace/rating-application/atomic-application/contract.rs"]
mod trace_contract_rating_application_atomic_application;

#[cfg(test)]
#[path = "trace/rating-application/quarantine-recovery/contract.rs"]
mod trace_contract_rating_application_quarantine_recovery;

#[cfg(test)]
#[path = "trace/rating-application/worker-process/contract.rs"]
mod trace_contract_rating_application_worker_process;
