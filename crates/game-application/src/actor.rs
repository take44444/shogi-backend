use crate::{
    Acceptance, Action, ApplicationError, GameId, GameRecord, GameRepository, MutationRequest,
    Participants, Receipt, RequestId, Snapshot, StagedGameChange, UserId,
};
use shogi_domain::Game;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{mpsc, oneshot};

pub(crate) enum Command {
    Snapshot(oneshot::Sender<Result<Snapshot, ApplicationError>>),
    Mutate {
        receipt: MutationRequest,
        reply: oneshot::Sender<Result<Acceptance, ApplicationError>>,
    },
}

struct State {
    game: Game,
    seq: u64,
    participants: Participants,
    accepted: HashMap<RequestId, Receipt>,
}

fn execute(
    game: &mut Game,
    participants: &Participants,
    user_id: &UserId,
    action: Action,
) -> Result<(), ApplicationError> {
    let side = participants.side(user_id)?;
    match action {
        Action::Move(movement) => {
            game.play(side, movement)
                .map_err(ApplicationError::Domain)?;
        }
        Action::Resign => {
            game.resign(side).map_err(ApplicationError::Domain)?;
        }
    }
    Ok(())
}

fn next_sequence(seq: u64) -> Result<u64, ApplicationError> {
    if seq >= i64::MAX as u64 {
        return Err(ApplicationError::SequenceExhausted);
    }
    Ok(seq + 1)
}

impl State {
    fn restore(record: GameRecord) -> Result<Self, ApplicationError> {
        let mut state = Self {
            game: Game::new(),
            seq: 0,
            participants: record.participants,
            accepted: HashMap::new(),
        };
        if record.version > i64::MAX as u64 {
            return Err(ApplicationError::Corrupt);
        }
        for receipt in record.journal {
            if receipt.request.expected_seq != state.seq
                || receipt.acceptance.seq
                    != next_sequence(state.seq).map_err(|_| ApplicationError::Corrupt)?
                || state.accepted.contains_key(&receipt.request.request_id)
            {
                return Err(ApplicationError::Corrupt);
            }
            execute(
                &mut state.game,
                &state.participants,
                &receipt.request.user_id,
                receipt.request.action,
            )
            .map_err(|_| ApplicationError::Corrupt)?;
            state.seq += 1;
            if receipt.acceptance != state.acceptance() {
                return Err(ApplicationError::Corrupt);
            }
            state
                .accepted
                .insert(receipt.request.request_id.clone(), receipt);
        }
        if state.seq != record.version || state.game.result() != record.result {
            return Err(ApplicationError::Corrupt);
        }
        Ok(state)
    }

    fn acceptance(&self) -> Acceptance {
        Acceptance {
            seq: self.seq,
            in_check: self.game.in_check(),
            result: self.game.result(),
        }
    }

    fn snapshot(&self, id: GameId) -> Snapshot {
        Snapshot {
            game_id: id,
            participants: self.participants.clone(),
            seq: self.seq,
            position: self.game.position(),
            turn: self.game.turn(),
            in_check: self.game.in_check(),
            result: self.game.result(),
            legal_moves: self.game.legal_moves(),
        }
    }

    fn replay(&self, request: &MutationRequest) -> Result<Option<Acceptance>, ApplicationError> {
        self.participants.side(&request.user_id)?;
        self.accepted
            .get(&request.request_id)
            .map(|original| {
                if original.request == *request {
                    Ok(original.acceptance)
                } else {
                    Err(ApplicationError::RequestConflict)
                }
            })
            .transpose()
    }
}

pub(crate) struct Actor {
    id: GameId,
    repository: Arc<dyn GameRepository>,
    state: Option<State>,
}

impl Actor {
    pub(crate) fn new(id: GameId, repository: Arc<dyn GameRepository>) -> Self {
        Self {
            id,
            repository,
            state: None,
        }
    }

    async fn reload(&mut self) -> Result<(), ApplicationError> {
        self.state = None;
        let record = self.repository.load_game(&self.id).await?;
        if record.id != self.id {
            return Err(ApplicationError::Corrupt);
        }
        self.state = Some(State::restore(record)?);
        Ok(())
    }

    async fn refresh(&mut self) -> Result<(), ApplicationError> {
        let version = self.repository.version(&self.id).await?;
        if self.state.as_ref().is_none_or(|state| state.seq != version) {
            self.reload().await?;
        }
        Ok(())
    }

    async fn snapshot(&mut self) -> Result<Snapshot, ApplicationError> {
        self.refresh().await?;
        let state = self.state.as_ref().ok_or(ApplicationError::Internal)?;
        Ok(state.snapshot(self.id.clone()))
    }

    pub(crate) async fn run(mut self, mut receiver: mpsc::Receiver<Command>) {
        while let Some(command) = receiver.recv().await {
            match command {
                Command::Snapshot(reply) => {
                    let _ = reply.send(self.snapshot().await);
                }
                Command::Mutate { receipt, reply } => {
                    let _ = reply.send(self.apply(receipt).await);
                }
            }
        }
    }

    async fn apply(&mut self, pending: MutationRequest) -> Result<Acceptance, ApplicationError> {
        self.refresh().await?;
        let state = self.state.as_ref().ok_or(ApplicationError::Internal)?;
        if let Some(acceptance) = state.replay(&pending)? {
            return Ok(acceptance);
        }
        if pending.expected_seq != state.seq {
            return Err(ApplicationError::SequenceMismatch {
                expected: pending.expected_seq,
                actual: state.seq,
            });
        }
        let seq = next_sequence(state.seq)?;
        let mut candidate = state.game.clone();
        execute(
            &mut candidate,
            &state.participants,
            &pending.user_id,
            pending.action,
        )?;
        let receipt = Receipt {
            request: pending.clone(),
            acceptance: Acceptance {
                seq,
                in_check: candidate.in_check(),
                result: candidate.result(),
            },
        };
        let change = StagedGameChange::new(self.id.clone(), receipt)?;
        match self.repository.commit_change(&change).await {
            Ok(()) => {
                let receipt = change.receipt();
                let state = self.state.as_mut().ok_or(ApplicationError::Internal)?;
                state.game = candidate;
                state.seq = seq;
                state
                    .accepted
                    .insert(receipt.request.request_id.clone(), receipt.clone());
                Ok(receipt.acceptance)
            }
            Err(error) => {
                self.reload().await?;
                let state = self.state.as_ref().ok_or(ApplicationError::Internal)?;
                if let Some(acceptance) = state.replay(&pending)? {
                    return Ok(acceptance);
                }
                if matches!(error, ApplicationError::SequenceMismatch { .. }) {
                    return Err(ApplicationError::SequenceMismatch {
                        expected: pending.expected_seq,
                        actual: state.seq,
                    });
                }
                self.state = None;
                Err(ApplicationError::Unavailable)
            }
        }
    }
}

pub(crate) fn snapshot_from_record(record: GameRecord) -> Result<Snapshot, ApplicationError> {
    let id = record.id.clone();
    Ok(State::restore(record)?.snapshot(id))
}

#[cfg(test)]
mod tests;
