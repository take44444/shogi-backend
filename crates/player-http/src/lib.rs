//! Anonymous HTTP access to completed public game records.
use axum::{
    Router,
    extract::{
        Path, Query, State,
        rejection::{PathRejection, QueryRejection},
    },
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use game_record_application::{
    Detail, Error, Filters, FinishReason, GameId, GameRecords, Metadata, Page, Player,
    PublicAction, Rule, Search, UserId,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Semaphore;

#[derive(Clone)]
struct App {
    records: GameRecords,
    capacity: Arc<Semaphore>,
}
pub fn router(records: GameRecords) -> Router {
    Router::new()
        .route("/game-records", get(search))
        .route("/game-records/{id}", get(detail))
        .with_state(App {
            records,
            capacity: Arc::new(Semaphore::new(16)),
        })
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchQuery {
    participant: Option<String>,
    rule: Option<String>,
    winner: Option<String>,
    reason: Option<String>,
    from: i64,
    to: i64,
    min_ply: Option<u64>,
    max_ply: Option<u64>,
    limit: Option<u32>,
    cursor: Option<String>,
}
impl SearchQuery {
    fn parse(self) -> Result<Search, Error> {
        let participant = self
            .participant
            .map(UserId::new)
            .transpose()
            .map_err(|_| Error::InvalidQuery)?;
        let rule = self
            .rule
            .map(|s| match s.as_str() {
                "standard" => Ok(Rule::Standard),
                _ => Err(Error::InvalidQuery),
            })
            .transpose()?;
        let winner = self
            .winner
            .map(|s| match s.as_str() {
                "black" => Ok(Player::Black),
                "white" => Ok(Player::White),
                _ => Err(Error::InvalidQuery),
            })
            .transpose()?;
        let reason = self
            .reason
            .map(|s| match s.as_str() {
                "checkmate" => Ok(FinishReason::Checkmate),
                "resignation" => Ok(FinishReason::Resignation),
                _ => Err(Error::InvalidQuery),
            })
            .transpose()?;
        Search::new(
            Filters {
                participant,
                rule,
                winner,
                reason,
                from: self.from,
                to: self.to,
                min_ply: self.min_ply,
                max_ply: self.max_ply,
            },
            self.limit.unwrap_or(30),
            self.cursor.as_deref(),
        )
    }
}
async fn search(
    State(app): State<App>,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> Response {
    let result = async {
        let search = query.map_err(|_| Error::InvalidQuery)?.0.parse()?;
        let _permit = app.capacity.try_acquire().map_err(|_| Error::Unavailable)?;
        app.records.search(&search).await.map(PageDto::from)
    }
    .await;
    respond(result)
}
async fn detail(State(app): State<App>, path: Result<Path<String>, PathRejection>) -> Response {
    let result = async {
        let id = GameId::new(path.map_err(|_| Error::InvalidQuery)?.0)?;
        let _permit = app.capacity.try_acquire().map_err(|_| Error::Unavailable)?;
        app.records.detail(&id).await.map(DetailDto::from)
    }
    .await;
    respond(result)
}
fn respond<T: Serialize>(result: Result<T, Error>) -> Response {
    let mut response = match result {
        Ok(body) => match serde_json::to_vec(&body) {
            Ok(bytes) if bytes.len() <= 1_048_576 => {
                ([(header::CONTENT_TYPE, "application/json")], bytes).into_response()
            }
            _ => error(Error::HistoryTooLong),
        },
        Err(failure) => error(failure),
    };
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        "no-store".parse().expect("static header"),
    );
    response
}
fn error(error: Error) -> Response {
    let (status, code) = match error {
        Error::InvalidQuery => (StatusCode::BAD_REQUEST, "invalid_query"),
        Error::NotFound => (StatusCode::NOT_FOUND, "not_found"),
        Error::Corrupt => (StatusCode::INTERNAL_SERVER_ERROR, "invalid_record"),
        Error::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
        Error::HistoryTooLong => (StatusCode::UNPROCESSABLE_ENTITY, "history_too_long"),
    };
    (status, axum::Json(serde_json::json!({"error": code}))).into_response()
}
fn player(player: Player) -> &'static str {
    if player == Player::Black {
        "black"
    } else {
        "white"
    }
}
#[derive(Serialize)]
struct ResultDto {
    winner: &'static str,
    reason: &'static str,
}
#[derive(Serialize)]
struct MetadataDto {
    id: String,
    black: String,
    white: String,
    rule: &'static str,
    result: ResultDto,
    finished_at: i64,
    final_seq: u64,
    final_move_ply: u64,
}
impl From<Metadata> for MetadataDto {
    fn from(m: Metadata) -> Self {
        Self {
            id: m.id.as_str().into(),
            black: m.black.as_str().into(),
            white: m.white.as_str().into(),
            rule: "standard",
            result: ResultDto {
                winner: player(m.result.winner),
                reason: if m.result.reason == FinishReason::Checkmate {
                    "checkmate"
                } else {
                    "resignation"
                },
            },
            finished_at: m.finished_at,
            final_seq: m.final_seq,
            final_move_ply: m.final_move_ply,
        }
    }
}
#[derive(Serialize)]
struct PageDto {
    items: Vec<MetadataDto>,
    next_cursor: Option<String>,
}
impl From<Page> for PageDto {
    fn from(p: Page) -> Self {
        Self {
            items: p.items.into_iter().map(Into::into).collect(),
            next_cursor: p.next_cursor,
        }
    }
}
#[derive(Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum ActionDto {
    Move { usi: String },
    Resign,
}
#[derive(Serialize)]
struct OperationDto {
    seq: u64,
    player: &'static str,
    #[serde(flatten)]
    action: ActionDto,
}
#[derive(Serialize)]
struct DetailDto {
    metadata: MetadataDto,
    history: Vec<OperationDto>,
}
impl From<Detail> for DetailDto {
    fn from(d: Detail) -> Self {
        Self {
            metadata: d.metadata.into(),
            history: d
                .history
                .into_iter()
                .map(|o| OperationDto {
                    seq: o.seq,
                    player: player(o.player),
                    action: match o.action {
                        PublicAction::Move(m) => ActionDto::Move { usi: m.to_string() },
                        PublicAction::Resign => ActionDto::Resign,
                    },
                })
                .collect(),
        }
    }
}
#[cfg(test)]
mod tests;
