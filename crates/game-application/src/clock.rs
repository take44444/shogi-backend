use crate::ApplicationError;
use rating_domain::Pool;
use shogi_domain::Player;

/// Nonnegative UTC epoch milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(i64);
impl Timestamp {
    /// Validates a database or persisted timestamp.
    pub fn new(value: i64) -> Result<Self, ApplicationError> {
        if value < 0 {
            return Err(ApplicationError::Corrupt);
        }
        Ok(Self(value))
    }
    /// Integer UTC epoch milliseconds.
    pub fn get(self) -> i64 {
        self.0
    }
    fn add(self, duration: Milliseconds) -> Result<Self, ApplicationError> {
        Self::new(
            self.0
                .checked_add(duration.0)
                .ok_or(ApplicationError::Corrupt)?,
        )
    }
}
/// A nonnegative millisecond duration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Milliseconds(i64);
impl Milliseconds {
    /// Validates persisted durations.
    pub fn new(value: i64) -> Result<Self, ApplicationError> {
        if value < 0 {
            return Err(ApplicationError::Corrupt);
        }
        Ok(Self(value))
    }
    /// Integer milliseconds.
    pub fn get(self) -> i64 {
        self.0
    }
}
/// Immutable time control frozen when a game starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeControl {
    /// Initial main time for each side.
    pub initial_main_ms: Milliseconds,
    /// Fresh byoyomi allowance on each turn.
    pub byoyomi_ms: Milliseconds,
}
impl TimeControl {
    /// Selects the supported pool's time control.
    pub fn for_pool(pool: Pool) -> Self {
        Self {
            initial_main_ms: Milliseconds(match pool {
                Pool::TenMinute30 => 600_000,
                Pool::Byoyomi30 => 0,
            }),
            byoyomi_ms: Milliseconds(30_000),
        }
    }
    /// Checks a stored control is supported, independent of future pool changes.
    pub fn validate(self) -> Result<(), ApplicationError> {
        if self.byoyomi_ms.0 != 30_000 || ![0, 600_000].contains(&self.initial_main_ms.0) {
            return Err(ApplicationError::Corrupt);
        }
        Ok(())
    }
}
/// Durable running or final clock. The game owns the side to move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockState {
    /// Main balances frozen at the start of this turn.
    Running {
        /// Black then White.
        main_remaining_ms: [Milliseconds; 2],
        /// Beginning of the current turn.
        turn_started_at: Timestamp,
        /// Exclusive move acceptance deadline.
        deadline_at: Timestamp,
    },
    /// Final balances retained for all future reads.
    Stopped {
        /// Black then White.
        main_remaining_ms: [Milliseconds; 2],
        /// Black then White.
        byoyomi_remaining_ms: [Milliseconds; 2],
        /// Deadline for timeouts, decision time for other finishes.
        stopped_at: Timestamp,
    },
}
fn side(player: Player) -> usize {
    usize::from(player == Player::White)
}
impl ClockState {
    /// Starts Black's clock in the game creation transaction.
    pub fn start(control: TimeControl, at: Timestamp) -> Result<Self, ApplicationError> {
        control.validate()?;
        Self::running(control, [control.initial_main_ms; 2], Player::Black, at)
    }
    fn running(
        control: TimeControl,
        main: [Milliseconds; 2],
        turn: Player,
        at: Timestamp,
    ) -> Result<Self, ApplicationError> {
        Ok(Self::Running {
            main_remaining_ms: main,
            turn_started_at: at,
            deadline_at: at.add(main[side(turn)])?.add(control.byoyomi_ms)?,
        })
    }
    /// Last saved time, used to clamp backward database time.
    pub fn saved_at(self) -> Timestamp {
        match self {
            Self::Running {
                turn_started_at, ..
            } => turn_started_at,
            Self::Stopped { stopped_at, .. } => stopped_at,
        }
    }
    /// Deadline for indexing and timeout candidates.
    pub fn deadline(self) -> Option<Timestamp> {
        match self {
            Self::Running { deadline_at, .. } => Some(deadline_at),
            Self::Stopped { .. } => None,
        }
    }
    /// Checks expiry without changing state.
    pub fn expired(self, now: Timestamp) -> bool {
        self.deadline().is_some_and(|deadline| now >= deadline)
    }
    /// Charges the old side, then starts the next side or freezes final balances.
    pub fn advance(
        self,
        control: TimeControl,
        turn: Player,
        now: Timestamp,
        next: Option<Player>,
    ) -> Result<Self, ApplicationError> {
        let Self::Running {
            mut main_remaining_ms,
            turn_started_at,
            deadline_at,
        } = self
        else {
            return Err(ApplicationError::Corrupt);
        };
        let at = now.max(turn_started_at);
        let elapsed = at.0 - turn_started_at.0;
        let old_main = main_remaining_ms[side(turn)].0;
        main_remaining_ms[side(turn)] = Milliseconds((old_main - elapsed).max(0));
        if let Some(next) = next {
            if at >= deadline_at {
                return Err(ApplicationError::Corrupt);
            }
            return Self::running(control, main_remaining_ms, next, at);
        }
        let mut byoyomi = [control.byoyomi_ms; 2];
        byoyomi[side(turn)] =
            Milliseconds((control.byoyomi_ms.0 - (elapsed - old_main).max(0)).max(0));
        Ok(Self::Stopped {
            main_remaining_ms,
            byoyomi_remaining_ms: byoyomi,
            stopped_at: at.min(deadline_at),
        })
    }
    /// Validates restored values against the game and frozen control.
    pub fn validate(
        self,
        control: TimeControl,
        turn: Player,
        finished: bool,
    ) -> Result<(), ApplicationError> {
        control.validate()?;
        let main = match self {
            Self::Running {
                main_remaining_ms,
                turn_started_at,
                deadline_at,
            } if !finished => {
                if Self::running(control, main_remaining_ms, turn, turn_started_at)?.deadline()
                    != Some(deadline_at)
                {
                    return Err(ApplicationError::Corrupt);
                }
                main_remaining_ms
            }
            Self::Stopped {
                main_remaining_ms,
                byoyomi_remaining_ms,
                ..
            } if finished => {
                if byoyomi_remaining_ms
                    .iter()
                    .any(|v| v.0 > control.byoyomi_ms.0)
                {
                    return Err(ApplicationError::Corrupt);
                }
                main_remaining_ms
            }
            _ => return Err(ApplicationError::Corrupt),
        };
        if main.iter().any(|v| v.0 > control.initial_main_ms.0) {
            return Err(ApplicationError::Corrupt);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn at(n: i64) -> Timestamp {
        Timestamp::new(n).unwrap()
    }
    #[test]
    fn byoyomi_transition_boundary_and_reset() {
        let control = TimeControl::for_pool(Pool::TenMinute30);
        let clock =
            ClockState::running(control, [Milliseconds(5_000); 2], Player::Black, at(100)).unwrap();
        assert!(!clock.expired(at(35_099)));
        assert!(clock.expired(at(35_100)));
        let next = clock
            .advance(control, Player::Black, at(10_100), Some(Player::White))
            .unwrap();
        let next = next
            .advance(control, Player::White, at(10_101), Some(Player::Black))
            .unwrap();
        assert_eq!(next.deadline(), Some(at(40_101)));
        let stopped = clock
            .advance(control, Player::Black, at(10_100), None)
            .unwrap();
        assert_eq!(
            stopped,
            ClockState::Stopped {
                main_remaining_ms: [Milliseconds(0), Milliseconds(5_000)],
                byoyomi_remaining_ms: [Milliseconds(25_000), Milliseconds(30_000)],
                stopped_at: at(10_100)
            }
        );
        assert_eq!(
            clock
                .advance(control, Player::Black, at(99), Some(Player::White))
                .unwrap()
                .saved_at(),
            at(100)
        );
        assert_eq!(
            clock
                .advance(control, Player::Black, at(50_000), None)
                .unwrap()
                .saved_at(),
            at(35_100)
        );
    }
}
