//! Serializes each game's commands, validates durable replay, and reconciles uncertain commits.
//! Candidates become cached state only after commit confirmation or durable receipt lookup.

use crate::{
    Acceptance, Action, ApplicationError, GameId, GameRecord, GameRepository, Participants,
    Receipt, RequestId, Snapshot, UserId,
};
use shogi_domain::Game;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{mpsc, oneshot};

#[derive(Clone)]
pub(crate) struct Pending {
    pub request_id: RequestId,
    pub user_id: UserId,
    pub expected_seq: u64,
    pub action: Action,
}

pub(crate) enum Command {
    Snapshot(oneshot::Sender<Result<Snapshot, ApplicationError>>),
    Mutate {
        receipt: Pending,
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
    /// Replays a coherent journal from the standard position.
    /// Rejects equal bindings, out-of-range or noncontiguous sequences, duplicate
    /// requests, nonparticipants, illegal actions, receipt mismatches, and final
    /// version or result mismatches as `Corrupt`. No partial state escapes.
    fn restore(record: GameRecord) -> Result<Self, ApplicationError> {
        let mut state = Self {
            game: Game::new(),
            seq: 0,
            participants: record.participants,
            accepted: HashMap::new(),
        };
        if state.participants.black == state.participants.white || record.version > i64::MAX as u64
        {
            return Err(ApplicationError::Corrupt);
        }
        for receipt in record.journal {
            if receipt.expected_seq != state.seq
                || receipt.acceptance.seq
                    != next_sequence(state.seq).map_err(|_| ApplicationError::Corrupt)?
                || state.accepted.contains_key(&receipt.request_id)
            {
                return Err(ApplicationError::Corrupt);
            }
            execute(
                &mut state.game,
                &state.participants,
                &receipt.user_id,
                receipt.action,
            )
            .map_err(|_| ApplicationError::Corrupt)?;
            state.seq += 1;
            if receipt.acceptance != state.acceptance() {
                return Err(ApplicationError::Corrupt);
            }
            state.accepted.insert(receipt.request_id.clone(), receipt);
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

    /// Checks membership before looking up an accepted request.
    /// Returns its original acceptance only when user, expected sequence, and action
    /// match. Changed contents conflict; an unused ID remains available for a new command.
    fn replay(&self, receipt: &Pending) -> Result<Option<Acceptance>, ApplicationError> {
        self.participants.side(&receipt.user_id)?;
        self.accepted
            .get(&receipt.request_id)
            .map(|original| {
                if original.user_id == receipt.user_id
                    && original.expected_seq == receipt.expected_seq
                    && original.action == receipt.action
                {
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

    /// Invalidates cached state before loading and validating durable history.
    /// A failed read or replay leaves state invalid so a later refresh can recover.
    async fn reload(&mut self) -> Result<(), ApplicationError> {
        self.state = None;
        let record = self.repository.load_game(&self.id).await?;
        if record.id != self.id {
            return Err(ApplicationError::Corrupt);
        }
        self.state = Some(State::restore(record)?);
        Ok(())
    }

    /// Reads the durable version before every query or mutation.
    /// Reloads missing or changed state. Read errors propagate even with a cached game.
    async fn refresh(&mut self) -> Result<(), ApplicationError> {
        let version = self.repository.version(&self.id).await?;
        if self.state.as_ref().is_none_or(|state| state.seq != version) {
            self.reload().await?;
        }
        Ok(())
    }

    async fn snapshot(&mut self) -> Result<Snapshot, ApplicationError> {
        self.refresh().await?;
        let state = self.state.as_ref().ok_or(ApplicationError::Unavailable)?;
        Ok(Snapshot {
            game_id: self.id.clone(),
            participants: state.participants.clone(),
            seq: state.seq,
            position: state.game.position(),
            turn: state.game.turn(),
            in_check: state.game.in_check(),
            result: state.game.result(),
            legal_moves: state.game.legal_moves(),
        })
    }

    /// Drains queued commands until all senders close.
    /// A dropped response receiver does not cancel an enqueued command or transaction.
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

    /// Replays accepted requests before checking staleness or domain completion.
    /// Stages new commands on a clone, checks sequence capacity before mutation, and
    /// publishes only a confirmed commit. Every append failure reloads history to
    /// resolve exact replay, conflict, or a competing version. Otherwise returns
    /// `Unavailable` with invalid state; absence of a receipt does not prove rollback.
    async fn apply(&mut self, pending: Pending) -> Result<Acceptance, ApplicationError> {
        self.refresh().await?;
        let state = self.state.as_ref().ok_or(ApplicationError::Unavailable)?;
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
            request_id: pending.request_id.clone(),
            user_id: pending.user_id.clone(),
            expected_seq: pending.expected_seq,
            action: pending.action,
            acceptance: Acceptance {
                seq,
                in_check: candidate.in_check(),
                result: candidate.result(),
            },
        };
        match self.repository.append(&self.id, &receipt).await {
            Ok(()) => {
                let state = self.state.as_mut().ok_or(ApplicationError::Unavailable)?;
                state.game = candidate;
                state.seq = seq;
                state
                    .accepted
                    .insert(receipt.request_id.clone(), receipt.clone());
                Ok(receipt.acceptance)
            }
            Err(error) => {
                self.reload().await?;
                let state = self.state.as_ref().ok_or(ApplicationError::Unavailable)?;
                if let Some(acceptance) = state.replay(&pending)? {
                    return Ok(acceptance);
                }
                // This conflict occurs if another writer advances this game's version
                // between refresh and append; a single-writer setup prevents it.
                // With exclusive ownership enforced by lease/fencing, this would
                // indicate a violation of the single-writer invariant.
                if matches!(error, ApplicationError::SequenceMismatch { .. }) {
                    return Err(ApplicationError::SequenceMismatch {
                        expected: pending.expected_seq,
                        actual: state.seq,
                    });
                }
                // The receipt is absent now. An uncertain transaction is never retried here.
                self.state = None;
                Err(ApplicationError::Unavailable)
            }
        }
    }
}

#[cfg(test)]
mod tests;
