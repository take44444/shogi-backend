use async_trait::async_trait;
use shogi_domain::{GameResult, Move};

use crate::{
    Acceptance, ApplicationError, GameId, MakeMove, Participants, RequestId, Resign,
    StagedGameChange, UserId,
};

/// Access metadata read from the same durable view as a viewer snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    /// Any authenticated user may view the game.
    Public,
    /// Only the two participants may view the game.
    Private,
}

/// Metadata needed to decide whether a viewer can receive game data.
#[derive(Clone, Debug)]
pub struct ViewMetadata {
    /// Permanent game participants.
    pub participants: Participants,
    /// Current visibility.
    pub visibility: Visibility,
}

impl ViewMetadata {
    /// Checks the common game viewing policy.
    pub fn permits(&self, viewer: &UserId) -> bool {
        self.visibility == Visibility::Public || self.participants.side(viewer).is_ok()
    }
}

/// One coherent view of access metadata and accepted history.
#[derive(Clone, Debug)]
pub struct ViewerGameRecord {
    /// Current access metadata.
    pub access: ViewMetadata,
    /// Complete accepted history from the same read.
    pub record: GameRecord,
}

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
    /// Locked database decision time.
    pub evaluated_at: crate::Timestamp,
    /// The original request, compared as one value during replay.
    pub request: MutationRequest,
    /// The original response; its sequence is `expected_seq + 1`.
    pub acceptance: Acceptance,
}

/// One durable transition, with no fabricated command identity for system finishes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JournalEntry {
    /// An accepted user command and retained response.
    UserCommand(Receipt),
    /// A timeout, independently of any triggering request.
    Timeout {
        /// Previous durable sequence.
        expected_seq: u64,
        /// Side whose deadline expired.
        loser: shogi_domain::Player,
        /// Original persisted deadline.
        deadline_at: crate::Timestamp,
        /// Database time when adjudication ran.
        evaluated_at: crate::Timestamp,
        /// Frozen result and final clock.
        acceptance: Acceptance,
    },
}
impl JournalEntry {
    /// Final response-shaped transition state.
    pub fn acceptance(&self) -> Acceptance {
        match self {
            Self::UserCommand(r) => r.acceptance,
            Self::Timeout { acceptance, .. } => *acceptance,
        }
    }
    /// Previous durable version.
    pub fn expected_seq(&self) -> u64 {
        match self {
            Self::UserCommand(r) => r.request.expected_seq,
            Self::Timeout { expected_seq, .. } => *expected_seq,
        }
    }
    /// Original user request, absent for system transitions.
    pub fn request(&self) -> Option<&MutationRequest> {
        match self {
            Self::UserCommand(r) => Some(&r.request),
            Self::Timeout { .. } => None,
        }
    }
    /// Frozen database decision time.
    pub fn evaluated_at(&self) -> crate::Timestamp {
        match self {
            Self::UserCommand(r) => r.evaluated_at,
            Self::Timeout { evaluated_at, .. } => *evaluated_at,
        }
    }
}
impl From<Receipt> for JournalEntry {
    fn from(value: Receipt) -> Self {
        Self::UserCommand(value)
    }
}

/// A coherent durable game record to restore by replay from [`shogi_domain::Game::new`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameRecord {
    /// Frozen time control.
    pub time_control: crate::TimeControl,
    /// Initial Black clock, retained for replay.
    pub initial_clock: crate::ClockState,
    /// Current durable clock.
    pub clock: crate::ClockState,
    /// Database time associated with this read.
    pub server_now_ms: crate::Timestamp,
    /// The requested game ID.
    pub id: GameId,
    /// The immutable, distinct participant bindings.
    pub participants: Participants,
    /// The state transition count, equal to the final journal sequence.
    pub version: u64,
    /// The final result, which must match journal replay.
    pub result: Option<GameResult>,
    /// All user and system transitions in contiguous sequence order, starting at one.
    pub journal: Vec<JournalEntry>,
}

/// Asynchronous durable operations required by [`crate::GameService`].
///
/// Implementors preserve immutable participant bindings, unique game IDs,
/// and per-game uniqueness of accepted request IDs and sequence numbers. New games
/// have version zero, no result, and an empty journal. Durable versions are bounded
/// by `i64::MAX`. Reads must not fabricate a successful empty record for an unknown ID.
///
/// [`Self::load_game`] returns one coherent view. Mutations begin with
/// [`Self::begin_game_update`]. The application validates replay under its row lock,
/// samples database time, then commits one frozen transition and its events through
/// [`LockedGameUpdate::commit`]. Terminal transitions take the admission lock first.
/// An uncertain commit is reconciled from durable history before responding.
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
    /// Locks a game and reads its coherent history, then obtains database time.
    /// Terminal updates must acquire the shared admission lock before the game lock.
    async fn begin_game_update(
        &self,
        id: &GameId,
        terminal: bool,
    ) -> Result<Box<dyn LockedGameUpdate>, ApplicationError>;
    /// Bounded expired candidate scan; no transaction locks escape this call.
    async fn expired_games(&self, limit: u32) -> Result<Vec<GameId>, ApplicationError>;

    /// Loads visibility, participants and history from one coherent database view.
    /// Implementations should hide inaccessible and absent games equally.
    async fn load_game_for_viewer(
        &self,
        id: &GameId,
        viewer: &UserId,
    ) -> Result<ViewerGameRecord, ApplicationError>;

    /// Reads current visibility and participants without loading history.
    async fn view_metadata(
        &self,
        id: &GameId,
        viewer: &UserId,
    ) -> Result<ViewMetadata, ApplicationError>;
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
}

/// An adapter-owned transaction. Dropping without commit rolls back.
#[async_trait]
pub trait LockedGameUpdate: Send {
    /// State protected by this game's row lock.
    fn locked_record(&self) -> &GameRecord;
    /// Database time obtained after acquiring the row lock.
    async fn decision_time(&mut self) -> Result<crate::Timestamp, ApplicationError>;
    /// Whether the admission lock was acquired before the game lock.
    fn terminal_allowed(&self) -> bool;
    /// Saves a decision and all terminal side effects in the same transaction.
    async fn commit(self: Box<Self>, change: &StagedGameChange) -> Result<(), ApplicationError>;
    /// Explicit rollback before acquiring locks in terminal order.
    async fn rollback(self: Box<Self>) -> Result<(), ApplicationError>;
}
