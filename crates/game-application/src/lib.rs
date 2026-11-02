#![doc = include_str!("../README.md")]

mod actor;
mod types;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc, oneshot};

use actor::{Actor, Command, Mutation};
pub use types::*;

const MAILBOX_CAPACITY: usize = 64;

#[derive(Clone, Default)]
pub struct GameService {
    games: Arc<Mutex<HashMap<GameId, mpsc::Sender<Command>>>>,
}

impl GameService {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn create(&self, id: GameId) -> Result<Snapshot, ApplicationError> {
        let mut games = self
            .games
            .lock()
            .map_err(|_| ApplicationError::Unavailable)?;
        if games.contains_key(&id) {
            return Err(ApplicationError::AlreadyExists);
        }
        let actor = Actor::new(id.clone());
        let snapshot = actor.snapshot();
        let (sender, receiver) = mpsc::channel(MAILBOX_CAPACITY);
        games.insert(id, sender);
        tokio::spawn(actor.run(receiver));
        Ok(snapshot)
    }

    pub async fn snapshot(&self, id: &GameId) -> Result<Snapshot, ApplicationError> {
        let sender = self.sender(id)?;
        let (reply, response) = oneshot::channel();
        sender
            .send(Command::Snapshot(reply))
            .await
            .map_err(|_| ApplicationError::Unavailable)?;
        response.await.map_err(|_| ApplicationError::Unavailable)
    }

    pub async fn make_move(
        &self,
        id: &GameId,
        command: MakeMove,
    ) -> Result<Acceptance, ApplicationError> {
        self.mutate(
            id,
            command.request_id,
            Mutation::Move {
                expected_seq: command.expected_seq,
                player: command.player,
                movement: command.movement,
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
            command.request_id,
            Mutation::Resign {
                expected_seq: command.expected_seq,
                player: command.player,
            },
        )
        .await
    }

    fn sender(&self, id: &GameId) -> Result<mpsc::Sender<Command>, ApplicationError> {
        self.games
            .lock()
            .map_err(|_| ApplicationError::Unavailable)?
            .get(id)
            .cloned()
            .ok_or(ApplicationError::NotFound)
    }

    async fn mutate(
        &self,
        id: &GameId,
        request_id: RequestId,
        mutation: Mutation,
    ) -> Result<Acceptance, ApplicationError> {
        let sender = self.sender(id)?;
        let (reply, response) = oneshot::channel();
        sender
            .send(Command::Mutate {
                request_id,
                mutation,
                reply,
            })
            .await
            .map_err(|_| ApplicationError::Unavailable)?;
        response.await.map_err(|_| ApplicationError::Unavailable)?
    }
}
