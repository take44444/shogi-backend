use async_trait::async_trait;
use shogi_domain::{GameResult, Move};

use crate::{Acceptance, ApplicationError, GameId, Participants, RequestId, User, UserId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Move(Move),
    Resign,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub request_id: RequestId,
    pub user_id: UserId,
    pub expected_seq: u64,
    pub action: Action,
    pub acceptance: Acceptance,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameRecord {
    pub id: GameId,
    pub participants: Participants,
    pub version: u64,
    pub result: Option<GameResult>,
    pub journal: Vec<Receipt>,
}

/// Durable operations. A failed append may have committed. Callers must reconcile
/// using load_game before publishing a candidate or accepting another mutation.
#[async_trait]
pub trait Store: Send + Sync {
    async fn create_user(&self, id: UserId) -> Result<User, ApplicationError>;
    async fn get_user(&self, id: &UserId) -> Result<User, ApplicationError>;
    async fn create_game(
        &self,
        id: GameId,
        participants: Participants,
    ) -> Result<(), ApplicationError>;
    /// Metadata and ordered journal must come from one coherent database view.
    async fn load_game(&self, id: &GameId) -> Result<GameRecord, ApplicationError>;
    async fn version(&self, id: &GameId) -> Result<u64, ApplicationError>;
    /// Atomically compares expected_seq, advances metadata, and inserts the receipt.
    async fn append(&self, id: &GameId, receipt: &Receipt) -> Result<(), ApplicationError>;
}
