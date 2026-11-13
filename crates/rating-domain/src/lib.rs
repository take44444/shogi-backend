//! The two independent rating pools and deterministic integer Elo policy.
use shogi_domain::Player;
use std::{error::Error, fmt, num::NonZeroU64};

/// Supported time controls. Clocks are outside the rating policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pool {
    /// 600 seconds plus 30-second byoyomi.
    TenMinute30,
    /// Zero main time plus 30-second byoyomi.
    Byoyomi30,
}
impl Pool {
    /// Both supported pools in public response order.
    pub const ALL: [Self; 2] = [Self::TenMinute30, Self::Byoyomi30];
    /// Stable persisted and public name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TenMinute30 => "ten_minute_30",
            Self::Byoyomi30 => "byoyomi_30",
        }
    }
    /// Decode a stable name without accepting other time controls.
    pub fn parse(value: &str) -> Result<Self, ArithmeticError> {
        match value {
            "ten_minute_30" => Ok(Self::TenMinute30),
            "byoyomi_30" => Ok(Self::Byoyomi30),
            _ => Err(ArithmeticError),
        }
    }
}

/// A committed rating, or no completed rated games.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RatingState {
    /// Effective Elo 1500, exposed publicly as null and zero games.
    #[default]
    Unrated,
    /// A rating after at least one completed rated game.
    Applied(AppliedRating),
}
/// A validated applied state. Construct through [`RatingState::from_parts`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppliedRating {
    rating: i64,
    completed: NonZeroU64,
}
impl RatingState {
    /// Validate a nullable rating and PostgreSQL BIGINT count.
    pub fn from_parts(rating: Option<i64>, completed: i64) -> Result<Self, ArithmeticError> {
        match (rating, completed) {
            (None, 0) => Ok(Self::Unrated),
            (Some(rating), completed) if completed > 0 => Ok(Self::Applied(AppliedRating {
                rating,
                completed: NonZeroU64::new(completed as u64).ok_or(ArithmeticError)?,
            })),
            _ => Err(ArithmeticError),
        }
    }
    /// The committed value, excluding initial Elo for an unrated user.
    pub fn rating(self) -> Option<i64> {
        match self {
            Self::Unrated => None,
            Self::Applied(state) => Some(state.rating),
        }
    }
    /// Completed games, bounded by PostgreSQL BIGINT by construction or calculation.
    pub fn completed(self) -> u64 {
        match self {
            Self::Unrated => 0,
            Self::Applied(state) => state.completed.get(),
        }
    }
    /// Elo used for this game's calculation.
    pub fn effective(self) -> i64 {
        self.rating().unwrap_or(1500)
    }
    fn advance(self, delta: i64) -> Result<Self, ArithmeticError> {
        let count = self
            .completed()
            .checked_add(1)
            .filter(|n| *n <= i64::MAX as u64)
            .ok_or(ArithmeticError)?;
        Self::from_parts(
            Some(self.effective().checked_add(delta).ok_or(ArithmeticError)?),
            count as i64,
        )
    }
}
/// Both resulting states and the single rounded black delta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RatingChange {
    /// Black's new committed state.
    pub black: RatingState,
    /// White's new committed state.
    pub white: RatingState,
    /// White receives the exact negation.
    pub delta_black: i64,
}
/// Invalid persisted state or arithmetic outside the supported integer range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArithmeticError;
impl fmt::Display for ArithmeticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid or exhausted rating state")
    }
}
impl Error for ArithmeticError {}
/// Calculate K32 Elo, rounding Black's delta once and negating it for White.
pub fn calculate(
    black: RatingState,
    white: RatingState,
    winner: Player,
) -> Result<RatingChange, ArithmeticError> {
    let difference = (white.effective() as i128 - black.effective() as i128) as f64;
    let expected = 1.0 / (1.0 + 10_f64.powf((difference / 400.0).clamp(-308.0, 308.0)));
    let actual = if winner == Player::Black { 1.0 } else { 0.0 };
    let delta_black = (32.0 * (actual - expected)).round() as i64;
    Ok(RatingChange {
        black: black.advance(delta_black)?,
        white: white.advance(-delta_black)?,
        delta_black,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equal_players_gain_and_lose_sixteen() {
        let c = calculate(RatingState::Unrated, RatingState::Unrated, Player::Black).unwrap();
        assert_eq!(c.black.rating(), Some(1516));
        assert_eq!(c.white.rating(), Some(1484));
        assert_eq!(c.black.completed(), 1);
        assert_eq!(c.white.completed(), 1);
        assert_eq!(
            calculate(RatingState::Unrated, RatingState::Unrated, Player::White)
                .unwrap()
                .delta_black,
            -16
        );
    }
    #[test]
    fn expected_upsets_and_extremes_are_zero_sum_and_checked() {
        let state = |n| RatingState::from_parts(Some(n), 3).unwrap();
        for (b, w) in [(1500, 1900), (1900, 1500), (i64::MIN + 32, i64::MAX - 32)] {
            for winner in [Player::Black, Player::White] {
                let c = calculate(state(b), state(w), winner).unwrap();
                assert_eq!(
                    c.black.effective() as i128 + c.white.effective() as i128,
                    b as i128 + w as i128
                );
                assert!(c.delta_black.abs() <= 32);
            }
        }
        assert_eq!(
            calculate(state(1500), state(1900), Player::Black)
                .unwrap()
                .delta_black,
            29
        );
        assert!(calculate(state(i64::MAX), state(i64::MAX), Player::Black).is_err());
        assert!(
            calculate(
                RatingState::from_parts(Some(1500), i64::MAX).unwrap(),
                state(1500),
                Player::Black
            )
            .is_err()
        );
        assert!(RatingState::from_parts(Some(1500), 0).is_err());
        assert!(RatingState::from_parts(None, 1).is_err());
    }
}
