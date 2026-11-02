#![doc = include_str!("../README.md")]

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

#[derive(Clone)]
pub struct GameService {
    store: Arc<dyn Store>,
    games: Arc<Mutex<HashMap<GameId, mpsc::Sender<Command>>>>,
}

impl GameService {
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self {
            store,
            games: Arc::default(),
        }
    }

    pub async fn create_user(&self, id: UserId) -> Result<User, ApplicationError> {
        self.store.create_user(id).await
    }

    pub async fn get_user(&self, id: &UserId) -> Result<User, ApplicationError> {
        self.store.get_user(id).await
    }

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

    pub async fn snapshot(&self, id: &GameId) -> Result<Snapshot, ApplicationError> {
        let sender = self.sender(id).await?;
        let (reply, response) = oneshot::channel();
        sender
            .send(Command::Snapshot(reply))
            .await
            .map_err(|_| ApplicationError::Unavailable)?;
        response.await.map_err(|_| ApplicationError::Unavailable)?
    }

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
