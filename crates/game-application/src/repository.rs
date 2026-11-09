use async_trait::async_trait;
use shogi_domain::{GameResult, Move};

use crate::{
    Acceptance, ApplicationError, GameId, MakeMove, Participants, RequestId, Resign,
    StagedGameChange, UserId,
};

/// The typed action stored for exact retry matching and journal replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// A board move or drop, stored in typed form.
    Move(Move),
    /// A resignation by the receipt's user.
    Resign,
}

/// The complete identity of a mutation, preserved for exact retries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MutationRequest {
    /// The request ID, unique among accepted commands in this game.
    pub request_id: RequestId,
    /// The bound participant who issued the command.
    pub user_id: UserId,
    /// The durable version before this command.
    pub expected_seq: u64,
    /// The action used for replay and exact retry comparison.
    pub action: Action,
}

impl From<MakeMove> for MutationRequest {
    fn from(command: MakeMove) -> Self {
        let MakeMove {
            request_id,
            user_id,
            expected_seq,
            movement,
        } = command;
        Self {
            request_id,
            user_id,
            expected_seq,
            action: Action::Move(movement),
        }
    }
}
impl From<Resign> for MutationRequest {
    fn from(command: Resign) -> Self {
        let Resign {
            request_id,
            user_id,
            expected_seq,
        } = command;
        Self {
            request_id,
            user_id,
            expected_seq,
            action: Action::Resign,
        }
    }
}

/// One accepted command and its original response. Rejections have no receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    /// The original request, compared as one value during replay.
    pub request: MutationRequest,
    /// The original response; its sequence is `expected_seq + 1`.
    pub acceptance: Acceptance,
}

/// A coherent durable game record to restore by replay from [`shogi_domain::Game::new`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameRecord {
    /// The requested game ID.
    pub id: GameId,
    /// The immutable, distinct participant bindings.
    pub participants: Participants,
    /// The accepted command count, equal to the final journal sequence.
    pub version: u64,
    /// The final result, which must match journal replay.
    pub result: Option<GameResult>,
    /// All accepted receipts in ascending, contiguous sequence order, starting at one.
    pub journal: Vec<Receipt>,
}

/// Asynchronous durable operations required by [`crate::GameService`].
///
/// Implementors preserve immutable participant bindings, unique game IDs,
/// and per-game uniqueness of accepted request IDs and sequence numbers. New games
/// have version zero, no result, and an empty journal. Durable versions are bounded
/// by `i64::MAX`. Reads must not fabricate a successful empty record for an unknown ID.
///
/// [`Self::load_game`] must return one coherent view. [`Self::commit_change`] must atomically
/// compare the version, advance metadata, insert exactly one receipt, and enqueue
/// the supplied [`StagedGameChange::events`] in their existing order. An `Ok`
/// result confirms durability. An error can follow a commit; callers must reconcile
/// by loading durable history before publishing a candidate or accepting another mutation.
/// The application stages legal commands and validates replay rather than trusting
/// stored position strings. Repository implementations still enforce storage constraints
/// and membership independently of the actor cache.
///
/// A game repository accepts user IDs from its caller and does not manage accounts.
///
/// ```compile_fail,E0599
/// use game_application::{GameRepository, UserId};
///
/// async fn register(repository: &dyn GameRepository, id: UserId) {
///     repository.create_user(id).await;
/// }
/// ```
#[async_trait]
pub trait GameRepository: Send + Sync {
    /// Atomically creates version-zero metadata with two distinct user IDs.
    /// Participant bindings cannot subsequently change.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::AlreadyExists`] for a duplicate game, or
    /// [`ApplicationError::Unavailable`] for a storage failure.
    async fn create_game(
        &self,
        id: GameId,
        participants: Participants,
    ) -> Result<(), ApplicationError>;
    /// Loads metadata and the complete journal from one coherent database view.
    /// The application checks binding, legality, request uniqueness, every receipt,
    /// and final metadata during restoration.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::NotFound`] for an unknown game,
    /// [`ApplicationError::Corrupt`] for invalid durable encoding, or
    /// [`ApplicationError::Unavailable`] for a failed read.
    async fn load_game(&self, id: &GameId) -> Result<GameRecord, ApplicationError>;
    /// Reads the current durable version even when a caller has cached state.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::NotFound`] for an unknown game,
    /// [`ApplicationError::Corrupt`] for invalid durable encoding, or
    /// [`ApplicationError::Unavailable`] for a failed read.
    async fn version(&self, id: &GameId) -> Result<u64, ApplicationError>;
    /// Atomically persists the supplied receipt, events, and resulting metadata.
    ///
    /// Compares the current version with [`MutationRequest::expected_seq`], verifies
    /// participant membership, and preserves per-game request and sequence uniqueness.
    /// The updated version, result, receipt, and all supplied events must commit
    /// together or not at all. The application generates events before calling this
    /// method. Implementors persist them without deriving or changing the batch.
    /// This operation is not itself an exact-retry API; the application reconciles
    /// failures through [`Self::load_game`]. It supplies a legally staged change.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::NotFound`] for an unknown game,
    /// [`ApplicationError::SequenceMismatch`] for a competing version,
    /// [`ApplicationError::NonParticipant`] for an unbound user, or
    /// [`ApplicationError::Unavailable`] for storage failures, including constraint
    /// or uncertain commit failures. An error does not prove rollback.
    async fn commit_change(&self, change: &StagedGameChange) -> Result<(), ApplicationError>;
}
