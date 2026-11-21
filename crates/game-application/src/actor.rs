use crate::{
    Acceptance, Action, ApplicationError, ClockState, GameId, GameRecord, GameRepository,
    JournalEntry, MutationRequest, Participants, Receipt, RequestId, Snapshot, StagedGameChange,
    TimeControl, Timestamp, UserId,
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
    time_control: TimeControl,
    clock: ClockState,
}
fn execute(
    game: &mut Game,
    participants: &Participants,
    user: &UserId,
    action: Action,
) -> Result<(), ApplicationError> {
    let side = participants.side(user)?;
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
            time_control: record.time_control,
            clock: record.initial_clock,
        };
        state
            .clock
            .validate(state.time_control, state.game.turn(), false)?;
        if state.clock != ClockState::start(state.time_control, state.clock.saved_at())?
            || record.version > i64::MAX as u64
        {
            return Err(ApplicationError::Corrupt);
        }
        for entry in record.journal {
            let acceptance = entry.acceptance();
            if entry.expected_seq() != state.seq
                || acceptance.seq
                    != next_sequence(state.seq).map_err(|_| ApplicationError::Corrupt)?
                || entry.evaluated_at() < state.clock.saved_at()
            {
                return Err(ApplicationError::Corrupt);
            }
            let turn = state.game.turn();
            match &entry {
                JournalEntry::UserCommand(r) => {
                    if state.accepted.contains_key(&r.request.request_id)
                        || state.clock.expired(r.evaluated_at)
                    {
                        return Err(ApplicationError::Corrupt);
                    }
                    execute(
                        &mut state.game,
                        &state.participants,
                        &r.request.user_id,
                        r.request.action,
                    )
                    .map_err(|_| ApplicationError::Corrupt)?;
                }
                JournalEntry::Timeout {
                    loser,
                    deadline_at,
                    evaluated_at,
                    ..
                } => {
                    if *loser != turn
                        || state.clock.deadline() != Some(*deadline_at)
                        || !state.clock.expired(*evaluated_at)
                    {
                        return Err(ApplicationError::Corrupt);
                    }
                    state
                        .game
                        .timeout()
                        .map_err(|_| ApplicationError::Corrupt)?;
                }
            }
            state.clock = state.clock.advance(
                state.time_control,
                turn,
                entry.evaluated_at(),
                state.game.result().is_none().then(|| state.game.turn()),
            )?;
            state.seq += 1;
            if acceptance != state.acceptance()
                || (state.seq == i64::MAX as u64 && state.game.result().is_none())
            {
                return Err(ApplicationError::Corrupt);
            }
            if let JournalEntry::UserCommand(r) = entry {
                state.accepted.insert(r.request.request_id.clone(), r);
            }
        }
        if state.seq != record.version
            || state.game.result() != record.result
            || state.clock != record.clock
        {
            return Err(ApplicationError::Corrupt);
        }
        Ok(state)
    }
    fn acceptance(&self) -> Acceptance {
        Acceptance {
            seq: self.seq,
            in_check: self.game.in_check(),
            result: self.game.result(),
            clock: self.clock,
        }
    }
    fn snapshot(&self, id: GameId, now: Timestamp) -> Snapshot {
        Snapshot {
            game_id: id,
            participants: self.participants.clone(),
            seq: self.seq,
            position: self.game.position(),
            turn: self.game.turn(),
            in_check: self.game.in_check(),
            result: self.game.result(),
            legal_moves: self.game.legal_moves(),
            time_control: self.time_control,
            clock: self.clock,
            server_now_ms: now.max(self.clock.saved_at()),
        }
    }
    fn replay(&self, request: &MutationRequest) -> Result<Option<Acceptance>, ApplicationError> {
        self.participants.side(&request.user_id)?;
        self.accepted
            .get(&request.request_id)
            .map(|r| {
                if r.request == *request {
                    Ok(r.acceptance)
                } else {
                    Err(ApplicationError::RequestConflict)
                }
            })
            .transpose()
    }
    fn stage(
        &mut self,
        id: GameId,
        request: Option<&MutationRequest>,
        at: Timestamp,
    ) -> Result<Option<StagedGameChange>, ApplicationError> {
        let at = at.max(self.clock.saved_at());
        let turn = self.game.turn();
        let expired = self.game.result().is_none() && self.clock.expired(at);
        let entry = if expired {
            let seq = next_sequence(self.seq)?;
            let deadline_at = self.clock.deadline().ok_or(ApplicationError::Corrupt)?;
            self.game.timeout().map_err(ApplicationError::Domain)?;
            self.clock = self.clock.advance(self.time_control, turn, at, None)?;
            self.seq = seq;
            JournalEntry::Timeout {
                expected_seq: seq - 1,
                loser: turn,
                deadline_at,
                evaluated_at: at,
                acceptance: self.acceptance(),
            }
        } else if let Some(request) = request {
            if request.expected_seq != self.seq {
                return Err(ApplicationError::SequenceMismatch {
                    expected: request.expected_seq,
                    actual: self.seq,
                });
            }
            let seq = next_sequence(self.seq)?;
            execute(
                &mut self.game,
                &self.participants,
                &request.user_id,
                request.action,
            )?;
            if seq == i64::MAX as u64 && self.game.result().is_none() {
                return Err(ApplicationError::SequenceExhausted);
            }
            self.clock = self.clock.advance(
                self.time_control,
                turn,
                at,
                self.game.result().is_none().then(|| self.game.turn()),
            )?;
            self.seq = seq;
            Receipt {
                request: request.clone(),
                acceptance: self.acceptance(),
                evaluated_at: at,
            }
            .into()
        } else {
            return Ok(None);
        };
        Ok(Some(StagedGameChange::from_entry(id, entry)?))
    }
}
pub(crate) struct Actor {
    id: GameId,
    repository: Arc<dyn GameRepository>,
}
impl Actor {
    pub(crate) fn new(id: GameId, repository: Arc<dyn GameRepository>) -> Self {
        Self { id, repository }
    }
    pub(crate) async fn run(self, mut receiver: mpsc::Receiver<Command>) {
        while let Some(command) = receiver.recv().await {
            match command {
                Command::Snapshot(reply) => {
                    let _ = reply.send(
                        self.repository
                            .load_game(&self.id)
                            .await
                            .and_then(snapshot_from_record),
                    );
                }
                Command::Mutate { receipt, reply } => {
                    let _ = reply.send(
                        update_game(&self.repository, &self.id, Some(receipt))
                            .await
                            .and_then(|r| r.ok_or(ApplicationError::Internal)),
                    );
                }
            }
        }
    }
}
pub(crate) async fn update_game(
    repository: &Arc<dyn GameRepository>,
    id: &GameId,
    pending: Option<MutationRequest>,
) -> Result<Option<Acceptance>, ApplicationError> {
    let mut terminal = false;
    loop {
        let mut locked = repository.begin_game_update(id, terminal).await?;
        let mut state = State::restore(locked.locked_record().clone())?;
        if let Some(request) = &pending
            && let Some(acceptance) = state.replay(request)?
        {
            locked.rollback().await?;
            return Ok(Some(acceptance));
        }
        let Some(change) =
            state.stage(id.clone(), pending.as_ref(), locked.decision_time().await?)?
        else {
            locked.rollback().await?;
            return Ok(None);
        };
        if change.entry().acceptance().result.is_some() && !locked.terminal_allowed() {
            locked.rollback().await?;
            terminal = true;
            continue;
        }
        let timeout = matches!(change.entry(), JournalEntry::Timeout { .. });
        match locked.commit(&change).await {
            Ok(()) => {
                return if timeout && pending.is_some() {
                    Err(ApplicationError::Domain(
                        shogi_domain::CommandError::GameFinished,
                    ))
                } else {
                    Ok(Some(change.entry().acceptance()))
                };
            }
            Err(error) => {
                let record = repository.load_game(id).await?;
                let state = State::restore(record)?;
                if let Some(request) = &pending
                    && let Some(acceptance) = state.replay(request)?
                {
                    return Ok(Some(acceptance));
                }
                if timeout && state.game.result().is_some() {
                    return if pending.is_some() {
                        Err(ApplicationError::Domain(
                            shogi_domain::CommandError::GameFinished,
                        ))
                    } else {
                        Ok(Some(state.acceptance()))
                    };
                }
                return Err(match error {
                    ApplicationError::SequenceMismatch { .. } => {
                        ApplicationError::SequenceMismatch {
                            expected: pending
                                .as_ref()
                                .map_or(change.entry().expected_seq(), |r| r.expected_seq),
                            actual: state.seq,
                        }
                    }
                    _ => ApplicationError::Unavailable,
                });
            }
        }
    }
}
pub(crate) fn snapshot_from_record(record: GameRecord) -> Result<Snapshot, ApplicationError> {
    let id = record.id.clone();
    let now = record.server_now_ms;
    Ok(State::restore(record)?.snapshot(id, now))
}
#[cfg(test)]
mod tests;
