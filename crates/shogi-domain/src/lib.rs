//! Standard shogi rules with owned domain types and strict USI notation.
//!
//! [`Game`] starts with Black to move. [`Move`] and [`Square`] parse and display
//! USI syntax. Parsing checks syntax only; [`Game::play`] checks position legality.
//! [`PositionView`] provides an owned snapshot without a position editor or importer.
//!
//! Rules include promotion, capture and demotion into hand, drops, nifu,
//! king safety, check evasion, and pawn-drop mate rejection. A checked player
//! without legal moves loses. A player without moves who is not checked does
//! not automatically lose. Repetition, perpetual check, entering king,
//! declaration, and time controls are not adjudicated.
//!
//! ```
//! use shogi_domain::{Game, Piece, PieceKind, Player, Square};
//!
//! let mut game = Game::new();
//! assert_eq!(game.legal_moves().len(), 30);
//! let outcome = game.play(Player::Black, "7g7f".parse()?)?;
//! assert!(!outcome.in_check);
//! assert_eq!(game.turn(), Player::White);
//! assert_eq!(game.position().piece_at(Square::new(7, 6)?),
//!     Some(Piece { owner: Player::Black, kind: PieceKind::Pawn }));
//! let result = game.resign(Player::Black)?; // Resignation is allowed off turn.
//! assert_eq!(result.winner, Player::White);
//! assert!(game.legal_moves().is_empty());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
#![warn(missing_docs)]

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
    /// Creates the standard initial position with Black to move and no result.
    pub fn new() -> Self {
        Self {
            rules: adapter::Rules::new(),
            result: None,
        }
    }

    /// Applies a legal move and returns the resulting check status and result.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError::GameFinished`] before checking turn or legality,
    /// [`CommandError::WrongTurn`] for the other side, [`CommandError::IllegalMove`]
    /// for an illegal move, or [`CommandError::RulesFailure`] if the rules engine
    /// cannot apply the move. Every error preserves the entire game.
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
    /// Ends the game with the opponent as winner, retaining the position and turn.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError::GameFinished`] if a result already exists.
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
    /// Returns the terminal result, or `None` while play continues.
    pub fn result(&self) -> Option<GameResult> {
        self.result
    }
    /// Whether the side to move is checked in the retained position.
    pub fn in_check(&self) -> bool {
        self.rules.in_check()
    }
    /// Copies the current board and hands into an owned snapshot.
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
