#![doc = include_str!("../README.md")]

mod adapter;
mod notation;
mod types;

pub use notation::ParseUsiError;
pub use types::*;

/// A standard shogi game. Only accepted moves and resignation mutate it.
/// No clock, persistence, repetition, or declaration adjudication is performed.
#[derive(Clone)]
pub struct Game {
    rules: adapter::Rules,
    result: Option<GameResult>,
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

impl Game {
    pub fn new() -> Self {
        Self {
            rules: adapter::Rules::new(),
            result: None,
        }
    }

    /// Rejects finished games before checking the player or move.
    /// Every error leaves the entire game unchanged.
    pub fn play(&mut self, player: Player, mv: Move) -> Result<MoveOutcome, CommandError> {
        if self.result.is_some() {
            return Err(CommandError::GameFinished);
        }
        if self.turn() != player {
            return Err(CommandError::WrongTurn);
        }
        self.rules.play(mv)?;
        let in_check = self.in_check();
        if in_check && self.rules.legal_moves().is_empty() {
            self.result = Some(GameResult {
                winner: player,
                reason: FinishReason::Checkmate,
            });
        }
        Ok(MoveOutcome {
            in_check,
            result: self.result,
        })
    }

    /// Either player may resign during an ongoing game, including off turn.
    pub fn resign(&mut self, player: Player) -> Result<GameResult, CommandError> {
        if self.result.is_some() {
            return Err(CommandError::GameFinished);
        }
        let result = GameResult {
            winner: player.opponent(),
            reason: FinishReason::Resignation,
        };
        self.result = Some(result);
        Ok(result)
    }

    /// The side to move in the retained position, even after the game ends.
    pub fn turn(&self) -> Player {
        self.rules.turn()
    }
    pub fn result(&self) -> Option<GameResult> {
        self.result
    }
    /// Whether the side to move is checked in the retained position.
    pub fn in_check(&self) -> bool {
        self.rules.in_check()
    }
    pub fn position(&self) -> PositionView {
        self.rules.position()
    }

    /// Includes optional nonpromotions. Finished games return no moves.
    pub fn legal_moves(&self) -> Vec<Move> {
        if self.result.is_some() {
            Vec::new()
        } else {
            self.rules.legal_moves()
        }
    }

    #[cfg(test)]
    fn from_sfen(sfen: &str) -> Self {
        Self {
            rules: adapter::Rules::from_sfen(sfen),
            result: None,
        }
    }
}

#[cfg(test)]
mod tests;
