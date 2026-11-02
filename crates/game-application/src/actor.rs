use crate::{Acceptance, ApplicationError, GameId, RequestId, Snapshot};
use shogi_domain::{Game, Move, Player};
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};

pub(crate) enum Command {
    Snapshot(oneshot::Sender<Snapshot>),
    Mutate {
        request_id: RequestId,
        mutation: Mutation,
        reply: oneshot::Sender<Result<Acceptance, ApplicationError>>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mutation {
    Move {
        expected_seq: u64,
        player: Player,
        movement: Move,
    },
    Resign {
        expected_seq: u64,
        player: Player,
    },
}

pub(crate) struct Actor {
    id: GameId,
    game: Game,
    seq: u64,
    accepted: HashMap<RequestId, (Mutation, Acceptance)>,
}

impl Actor {
    pub(crate) fn new(id: GameId) -> Self {
        Self {
            id,
            game: Game::new(),
            seq: 0,
            accepted: HashMap::new(),
        }
    }

    pub(crate) fn snapshot(&self) -> Snapshot {
        Snapshot {
            game_id: self.id.clone(),
            seq: self.seq,
            position: self.game.position(),
            turn: self.game.turn(),
            in_check: self.game.in_check(),
            result: self.game.result(),
            legal_moves: self.game.legal_moves(),
        }
    }

    pub(crate) async fn run(mut self, mut receiver: mpsc::Receiver<Command>) {
        while let Some(command) = receiver.recv().await {
            match command {
                Command::Snapshot(reply) => {
                    let _ = reply.send(self.snapshot());
                }
                Command::Mutate {
                    request_id,
                    mutation,
                    reply,
                } => {
                    let _ = reply.send(self.apply(request_id, mutation));
                }
            }
        }
    }

    fn apply(
        &mut self,
        request_id: RequestId,
        mutation: Mutation,
    ) -> Result<Acceptance, ApplicationError> {
        if let Some((original, acceptance)) = self.accepted.get(&request_id) {
            return if *original == mutation {
                Ok(*acceptance)
            } else {
                Err(ApplicationError::RequestConflict)
            };
        }
        let expected = match mutation {
            Mutation::Move { expected_seq, .. } | Mutation::Resign { expected_seq, .. } => {
                expected_seq
            }
        };
        if expected != self.seq {
            return Err(ApplicationError::SequenceMismatch {
                expected,
                actual: self.seq,
            });
        }
        let seq = self
            .seq
            .checked_add(1)
            .ok_or(ApplicationError::SequenceExhausted)?;
        match mutation {
            Mutation::Move {
                player, movement, ..
            } => {
                self.game
                    .play(player, movement)
                    .map_err(ApplicationError::Domain)?;
            }
            Mutation::Resign { player, .. } => {
                self.game.resign(player).map_err(ApplicationError::Domain)?;
            }
        }
        self.seq = seq;
        let acceptance = Acceptance {
            seq,
            in_check: self.game.in_check(),
            result: self.game.result(),
        };
        self.accepted.insert(request_id, (mutation, acceptance));
        Ok(acceptance)
    }
}

#[cfg(test)]
mod tests;
