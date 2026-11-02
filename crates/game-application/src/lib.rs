//! Durable shogi commands coordinated through the asynchronous [`Store`] port.
//!
//! [`GameService`] binds two existing users to a game and derives their sides
//! from [`Participants`]. User IDs are caller claims, not authentication credentials.
//! The application contains no database types and has no default in-memory store.
//!
//! Each service registry runs one actor per game with a 64-command mailbox.
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
//! Rejected request IDs remain reusable. Creating a user or game is not idempotent.
//!
//! A command stages a cloned game and receipt. The actor publishes that candidate
//! only after [`Store::append`] confirms a commit. Every failed append triggers a
//! durable reload. A matching receipt yields its original acceptance; changed
//! contents yield [`ApplicationError::RequestConflict`]. If append reported a version
//! conflict and no matching receipt exists, the actor returns
//! [`ApplicationError::SequenceMismatch`] with the reloaded version. Other append
//! failures without a receipt
//! yields [`ApplicationError::Unavailable`] and invalidates local state. Failed
//! reconciliation also leaves state invalid; a later successful refresh can recover.
//!
//! Cancellation after enqueue does not cancel the actor's transaction. A lost
//! response or `Unavailable` can mean the command committed. Retry identical
//! contents with the same request ID, including the original expected sequence.
#![warn(missing_docs)]

mod actor;
mod store;
mod types;

use actor::{Actor, Command, Pending};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
pub use store::*;
use tokio::sync::{mpsc, oneshot};
pub use types::*;

const MAILBOX_CAPACITY: usize = 64;

/// Cloneable application facade backed by durable storage and per-game actors.
///
/// See the [crate documentation](crate) for cancellation and retry contracts.
#[derive(Clone)]
pub struct GameService {
    store: Arc<dyn Store>,
    games: Arc<Mutex<HashMap<GameId, mpsc::Sender<Command>>>>,
}

impl GameService {
    /// Creates a service with an empty actor registry and the supplied durable store.
    /// No database access occurs until an operation is called.
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self {
            store,
            games: Arc::default(),
        }
    }

    /// Creates a user with the supplied internal ID. This does not authenticate anyone.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::AlreadyExists`] for a duplicate ID, or a store error.
    pub async fn create_user(&self, id: UserId) -> Result<User, ApplicationError> {
        self.store.create_user(id).await
    }

    /// Loads an existing user.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::UserNotFound`] for an unknown user, or a store error.
    pub async fn get_user(&self, id: &UserId) -> Result<User, ApplicationError> {
        self.store.get_user(id).await
    }

    /// Creates a standard game with permanent side bindings, then reads its snapshot.
    /// A snapshot failure can occur after game creation committed.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::InvalidParticipants`] for equal users,
    /// [`ApplicationError::UserNotFound`] for a missing user,
    /// [`ApplicationError::AlreadyExists`] for a duplicate game, or storage, actor,
    /// or restoration errors from [`Self::snapshot`].
    ///
    /// # Panics
    ///
    /// Panics if actor creation is needed outside a Tokio runtime.
    pub async fn create(
        &self,
        id: GameId,
        participants: Participants,
    ) -> Result<Snapshot, ApplicationError> {
        if participants.black == participants.white {
            return Err(ApplicationError::InvalidParticipants);
        }
        self.store.create_game(id.clone(), participants).await?;
        self.snapshot(&id).await
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
        self.mutate(
            id,
            Pending {
                request_id: command.request_id,
                user_id: command.user_id,
                expected_seq: command.expected_seq,
                action: Action::Move(command.movement),
            },
        )
        .await
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
        self.mutate(
            id,
            Pending {
                request_id: command.request_id,
                user_id: command.user_id,
                expected_seq: command.expected_seq,
                action: Action::Resign,
            },
        )
        .await
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
        self.store.version(id).await?;
        let mut games = self
            .games
            .lock()
            .map_err(|_| ApplicationError::Unavailable)?;
        if let Some(sender) = games.get(id).filter(|sender| !sender.is_closed()) {
            return Ok(sender.clone());
        }
        let (sender, receiver) = mpsc::channel(MAILBOX_CAPACITY);
        let actor = Actor::new(id.clone(), self.store.clone());
        games.insert(id.clone(), sender.clone());
        tokio::spawn(actor.run(receiver));
        Ok(sender)
    }

    async fn mutate(&self, id: &GameId, receipt: Pending) -> Result<Acceptance, ApplicationError> {
        let sender = self.sender(id).await?;
        let (reply, response) = oneshot::channel();
        sender
            .send(Command::Mutate { receipt, reply })
            .await
            .map_err(|_| ApplicationError::Unavailable)?;
        response.await.map_err(|_| ApplicationError::Unavailable)?
    }
}
