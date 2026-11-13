//! Bounded public completed-game search and history, independent of transport and storage.

pub mod ratings;

use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
pub use shogi_domain::{FinishReason, GameResult, Move, Player};
use std::{fmt, sync::Arc};
pub use user_id::UserId;

pub const MAX_HISTORY: u64 = 4096;
pub const MAX_PERIOD_MICROS: i64 = 366 * 24 * 60 * 60 * 1_000_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameId(String);
impl GameId {
    pub fn new(id: impl Into<String>) -> Result<Self, Error> {
        let id = id.into();
        if id.is_empty() || id.len() > 128 || id.chars().any(char::is_whitespace) {
            return Err(Error::InvalidQuery);
        }
        Ok(Self(id))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    Standard,
}

/// UTC microseconds, with an inclusive start and exclusive end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filters {
    pub participant: Option<UserId>,
    pub rule: Option<Rule>,
    pub winner: Option<Player>,
    pub reason: Option<FinishReason>,
    pub from: i64,
    pub to: i64,
    pub min_ply: Option<u64>,
    pub max_ply: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seek {
    pub finished_at: PreparedFinishTime,
    pub id: GameId,
}

#[derive(Clone, Debug)]
pub struct Search {
    filters: Filters,
    limit: u32,
    seek: Option<Seek>,
}
impl Search {
    pub fn new(filters: Filters, limit: u32, cursor: Option<&str>) -> Result<Self, Error> {
        if !(1..=100).contains(&limit)
            || filters.from < 0
            || filters.to <= filters.from
            || filters
                .to
                .checked_sub(filters.from)
                .is_none_or(|span| span > MAX_PERIOD_MICROS)
            || filters.min_ply.is_some_and(|n| n > i64::MAX as u64)
            || filters.max_ply.is_some_and(|n| n > i64::MAX as u64)
            || matches!((filters.min_ply, filters.max_ply), (Some(a), Some(b)) if a > b)
        {
            return Err(Error::InvalidQuery);
        }
        let seek = cursor
            .map(|cursor| {
                if cursor.len() > 1024 {
                    return Err(Error::InvalidQuery);
                }
                let bytes = URL_SAFE_NO_PAD
                    .decode(cursor)
                    .map_err(|_| Error::InvalidQuery)?;
                let (version, time, id, binding): (u8, i64, String, String) =
                    serde_json::from_slice(&bytes).map_err(|_| Error::InvalidQuery)?;
                if version != 1
                    || binding != fingerprint(&filters)
                    || time < filters.from
                    || time >= filters.to
                {
                    return Err(Error::InvalidQuery);
                }
                Ok(Seek {
                    finished_at: PreparedFinishTime::try_from(time)
                        .map_err(|_| Error::InvalidQuery)?,
                    id: GameId::new(id)?,
                })
            })
            .transpose()?;
        Ok(Self {
            filters,
            limit,
            seek,
        })
    }
    pub fn filters(&self) -> &Filters {
        &self.filters
    }
    pub fn limit(&self) -> u32 {
        self.limit
    }
    pub fn seek(&self) -> Option<&Seek> {
        self.seek.as_ref()
    }
    pub fn cursor(&self, last: &Metadata) -> String {
        URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&(
                1u8,
                last.finished_at().get(),
                last.id().as_str(),
                fingerprint(&self.filters),
            ))
            .expect("cursor tuple serializes"),
        )
    }
}
fn fingerprint(filters: &Filters) -> String {
    let fields = (
        filters.participant.as_ref().map(UserId::as_str),
        filters.rule.map(|_| "standard"),
        filters
            .winner
            .map(|p| if p == Player::Black { "black" } else { "white" }),
        filters.reason.map(|r| {
            if r == FinishReason::Checkmate {
                "checkmate"
            } else {
                "resignation"
            }
        }),
        filters.from,
        filters.to,
        filters.min_ply,
        filters.max_ply,
    );
    URL_SAFE_NO_PAD.encode(Sha256::digest(
        serde_json::to_vec(&fields).expect("filter tuple serializes"),
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct OperationSequence(u64);
impl OperationSequence {
    pub fn get(self) -> u64 {
        self.0
    }
}
impl TryFrom<u64> for OperationSequence {
    type Error = Error;
    fn try_from(value: u64) -> Result<Self, Error> {
        if value > i64::MAX as u64 {
            return Err(Error::Corrupt);
        }
        Ok(Self(value))
    }
}
impl TryFrom<i64> for OperationSequence {
    type Error = Error;
    fn try_from(value: i64) -> Result<Self, Error> {
        Self::try_from(u64::try_from(value).map_err(|_| Error::Corrupt)?)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MovePly(u64);
impl MovePly {
    pub fn get(self) -> u64 {
        self.0
    }
}
impl TryFrom<u64> for MovePly {
    type Error = Error;
    fn try_from(value: u64) -> Result<Self, Error> {
        if value > i64::MAX as u64 {
            return Err(Error::Corrupt);
        }
        Ok(Self(value))
    }
}
impl TryFrom<i64> for MovePly {
    type Error = Error;
    fn try_from(value: i64) -> Result<Self, Error> {
        Self::try_from(u64::try_from(value).map_err(|_| Error::Corrupt)?)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PreparedFinishTime(i64);
impl PreparedFinishTime {
    pub fn get(self) -> i64 {
        self.0
    }
}
impl TryFrom<i64> for PreparedFinishTime {
    type Error = Error;
    fn try_from(value: i64) -> Result<Self, Error> {
        if value < 0 {
            return Err(Error::Corrupt);
        }
        Ok(Self(value))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Metadata {
    id: GameId,
    black: UserId,
    white: UserId,
    rule: Rule,
    result: GameResult,
    finished_at: PreparedFinishTime,
    final_seq: OperationSequence,
    final_move_ply: MovePly,
}
impl Metadata {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: GameId,
        black: UserId,
        white: UserId,
        rule: Rule,
        result: GameResult,
        finished_at: PreparedFinishTime,
        final_seq: OperationSequence,
        final_move_ply: MovePly,
    ) -> Result<Self, Error> {
        if black == white
            || final_seq.get() == 0
            || final_move_ply.get()
                != final_seq.get() - u64::from(result.reason == FinishReason::Resignation)
        {
            return Err(Error::Corrupt);
        }
        Ok(Self {
            id,
            black,
            white,
            rule,
            result,
            finished_at,
            final_seq,
            final_move_ply,
        })
    }
    pub fn id(&self) -> &GameId {
        &self.id
    }
    pub fn black(&self) -> &UserId {
        &self.black
    }
    pub fn white(&self) -> &UserId {
        &self.white
    }
    pub fn rule(&self) -> Rule {
        self.rule
    }
    pub fn result(&self) -> GameResult {
        self.result
    }
    pub fn finished_at(&self) -> PreparedFinishTime {
        self.finished_at
    }
    pub fn final_seq(&self) -> OperationSequence {
        self.final_seq
    }
    pub fn final_move_ply(&self) -> MovePly {
        self.final_move_ply
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicAction {
    Move(Move),
    Resign,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicOperation {
    pub seq: OperationSequence,
    pub player: Player,
    pub action: PublicAction,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Detail {
    pub metadata: Metadata,
    pub history: Vec<PublicOperation>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    pub items: Vec<Metadata>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidQuery,
    NotFound,
    Corrupt,
    Unavailable,
    HistoryTooLong,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidQuery => "invalid game record query",
            Self::NotFound => "game record not found",
            Self::Corrupt => "game record is corrupt",
            Self::Unavailable => "game records unavailable",
            Self::HistoryTooLong => "game record exceeds history limit",
        })
    }
}
impl std::error::Error for Error {}

#[async_trait]
pub trait RecordRepository: Send + Sync + 'static {
    async fn search(&self, search: &Search) -> Result<Page, Error>;
    async fn detail(&self, id: &GameId) -> Result<Detail, Error>;
}
#[derive(Clone)]
pub struct GameRecords {
    repository: Arc<dyn RecordRepository>,
}
impl GameRecords {
    pub fn new(repository: Arc<dyn RecordRepository>) -> Self {
        Self { repository }
    }
    pub async fn search(&self, search: &Search) -> Result<Page, Error> {
        self.repository.search(search).await
    }
    pub async fn detail(&self, id: &GameId) -> Result<Detail, Error> {
        self.repository.detail(id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn filters() -> Filters {
        Filters {
            participant: None,
            rule: None,
            winner: None,
            reason: None,
            from: 0,
            to: 100,
            min_ply: None,
            max_ply: None,
        }
    }
    #[test]
    fn record_scalars_reject_values_outside_storage_bounds() {
        for value in [0u64, 1, i64::MAX as u64] {
            assert_eq!(OperationSequence::try_from(value).unwrap().get(), value);
            assert_eq!(MovePly::try_from(value).unwrap().get(), value);
        }
        for value in [i64::MAX as u64 + 1, u64::MAX] {
            assert_eq!(OperationSequence::try_from(value), Err(Error::Corrupt));
            assert_eq!(MovePly::try_from(value), Err(Error::Corrupt));
        }
        for value in [i64::MIN, -1] {
            assert_eq!(OperationSequence::try_from(value), Err(Error::Corrupt));
            assert_eq!(MovePly::try_from(value), Err(Error::Corrupt));
            assert_eq!(PreparedFinishTime::try_from(value), Err(Error::Corrupt));
        }
        for value in [0i64, 1, i64::MAX] {
            assert_eq!(
                OperationSequence::try_from(value).unwrap().get(),
                value as u64
            );
            assert_eq!(MovePly::try_from(value).unwrap().get(), value as u64);
            assert_eq!(PreparedFinishTime::try_from(value).unwrap().get(), value);
        }
    }

    #[test]
    fn metadata_construction_requires_distinct_players_and_consistent_finish_counts() {
        for (black, white, reason, seq, ply, valid) in [
            ("black", "white", FinishReason::Checkmate, 1u64, 1u64, true),
            ("black", "white", FinishReason::Resignation, 1, 0, true),
            ("black", "white", FinishReason::Resignation, 3, 2, true),
            ("black", "white", FinishReason::Checkmate, 0, 0, false),
            ("black", "white", FinishReason::Resignation, 0, 0, false),
            ("black", "black", FinishReason::Resignation, 1, 0, false),
            ("black", "white", FinishReason::Resignation, 1, 1, false),
            ("black", "white", FinishReason::Checkmate, 1, 0, false),
            ("black", "white", FinishReason::Checkmate, 1, 2, false),
            (
                "black",
                "white",
                FinishReason::Resignation,
                i64::MAX as u64,
                i64::MAX as u64 - 1,
                true,
            ),
            (
                "black",
                "white",
                FinishReason::Checkmate,
                i64::MAX as u64,
                i64::MAX as u64,
                true,
            ),
        ] {
            let metadata = Metadata::new(
                GameId::new("id").unwrap(),
                UserId::new(black).unwrap(),
                UserId::new(white).unwrap(),
                Rule::Standard,
                GameResult {
                    winner: Player::Black,
                    reason,
                },
                PreparedFinishTime::try_from(0).unwrap(),
                OperationSequence::try_from(seq).unwrap(),
                MovePly::try_from(ply).unwrap(),
            );
            assert_eq!(
                metadata.is_ok(),
                valid,
                "{black}, {white}, {reason:?}, {seq}, {ply}"
            );
            if let Ok(metadata) = metadata {
                assert_eq!(metadata.id().as_str(), "id");
                assert_eq!(metadata.black().as_str(), black);
                assert_eq!(metadata.white().as_str(), white);
                assert_eq!(metadata.rule(), Rule::Standard);
                assert_eq!(
                    metadata.result(),
                    GameResult {
                        winner: Player::Black,
                        reason
                    }
                );
                assert_eq!(metadata.finished_at().get(), 0);
                assert_eq!(metadata.final_seq().get(), seq);
                assert_eq!(metadata.final_move_ply().get(), ply);
            } else {
                assert_eq!(metadata, Err(Error::Corrupt));
            }
        }
    }

    #[test]
    fn bounds_and_cursor_binding() {
        for (from, to, min, max, limit) in [
            (0, 0, None, None, 30),
            (0, MAX_PERIOD_MICROS + 1, None, None, 30),
            (-1, 1, None, None, 30),
            (0, 100, Some(2), Some(1), 30),
            (0, 100, None, None, 101),
        ] {
            let mut f = filters();
            f.from = from;
            f.to = to;
            f.min_ply = min;
            f.max_ply = max;
            assert!(Search::new(f, limit, None).is_err());
        }
        let search = Search::new(filters(), 1, None).unwrap();
        let metadata = Metadata::new(
            GameId::new("id").unwrap(),
            UserId::new("black").unwrap(),
            UserId::new("white").unwrap(),
            Rule::Standard,
            GameResult {
                winner: Player::Black,
                reason: FinishReason::Resignation,
            },
            PreparedFinishTime::try_from(20).unwrap(),
            OperationSequence::try_from(1u64).unwrap(),
            MovePly::try_from(0u64).unwrap(),
        )
        .unwrap();
        let cursor = search.cursor(&metadata);
        assert_eq!(
            cursor,
            "WzEsMjAsImlkIiwiOFNDYU54TWdkOElJOVlnbVpKZ2gzZWctd0xmMTM5YW9XdkYzRWZ1bjJNayJd"
        );
        let decoded = Search::new(filters(), 100, Some(&cursor)).unwrap();
        let seek = decoded.seek().unwrap();
        assert_eq!(seek.finished_at.get(), 20);
        assert_eq!(seek.id.as_str(), "id");
        assert!(Search::new(filters(), 100, Some(&cursor)).is_ok());
        let mut changed = filters();
        changed.participant = Some(UserId::new("black").unwrap());
        assert!(Search::new(changed, 1, Some(&cursor)).is_err());
        assert!(Search::new(filters(), 1, Some("garbage")).is_err());
    }
}
