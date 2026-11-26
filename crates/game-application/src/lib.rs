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
//! Snapshots restore one coherent durable record. Mutations restore the record under
//! its database row lock, validate membership and exact retries, then sample database
//! time. Expired clocks finish before new command validation. Terminal updates acquire
//! the shared admission lock before the game lock, rolling back and retrying if needed.
//! Dropping the last service closes senders; actors drain queued commands and exit.
//!
//! # Sequences and retries
//!
//! Each accepted command or system timeout advances the state transition sequence.
//! The final `i64::MAX` slot is reserved for a terminal transition. Only accepted user
//! commands reserve request IDs. Exact retries match the whole request and return the
//! original acceptance and clock, even after restart or a subsequent timeout.
//! A timeout never reserves the triggering user's request ID.
//!
//! The application freezes a journal entry, clock and canonical events. The locked
//! repository transaction saves them with result, participation and rating job changes.
//! An uncertain commit is reconciled by restoring durable history before responding.
//! No cross-command position cache is retained, so separate processes always decide
//! against the same locked record. Failed reads fail closed.
//!
//! Cancellation after enqueue does not cancel the actor's transaction. A lost
//! response or `Unavailable` can mean the command committed. Retry identical
//! contents with the same request ID, including the original expected sequence.
#![warn(missing_docs)]

mod actor;
mod clock;
pub use clock::*;
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

    /// Adjudicates bounded expired games, including games with no loaded actor.
    pub async fn poll_timeouts(&self, limit: u32) -> Result<usize, ApplicationError> {
        let ids = self.repository.expired_games(limit).await?;
        let mut finished = 0;
        let mut failure = None;
        for id in ids {
            match actor::update_game(&self.repository, &id, None).await {
                Ok(Some(_)) => finished += 1,
                Ok(None) => {}
                Err(error) => {
                    failure.get_or_insert(error);
                }
            }
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(finished),
        }
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
