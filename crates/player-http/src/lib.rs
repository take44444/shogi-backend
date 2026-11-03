//! Anonymous HTTP access to completed public game records.
use axum::{
    Router,
    extract::{
        Path, Query, State,
        rejection::{PathRejection, QueryRejection},
    },
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use game_record_application::{
    Detail, Error, Filters, FinishReason, GameId, GameRecords, Metadata, Page, Player,
    PublicAction, Rule, Search, UserId,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Semaphore;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(Clone)]
struct App {
    records: GameRecords,
    capacity: Arc<Semaphore>,
}
/// Construct the document without a database, configuration, or service startup.
pub fn openapi() -> utoipa::openapi::OpenApi {
    api_router().into_openapi()
}
fn api_router() -> OpenApiRouter<App> {
    let document = utoipa::openapi::OpenApi::new(
        utoipa::openapi::Info::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
        utoipa::openapi::Paths::new(),
    );
    OpenApiRouter::with_openapi(document)
        .routes(routes!(search))
        .routes(routes!(detail))
        .routes(routes!(openapi_json))
}
pub fn router(records: GameRecords) -> Router {
    api_router().split_for_parts().0.with_state(App {
        records,
        capacity: Arc::new(Semaphore::new(16)),
    })
}
#[utoipa::path(get, path = "/openapi.json", responses(
    (status = 200, description = "OpenAPI document", body = Object, content_type = "application/json")
))]
async fn openapi_json() -> Response {
    respond(Ok(openapi()))
}
#[derive(Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
enum RuleDto {
    Standard,
}
#[derive(Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
enum PlayerDto {
    Black,
    White,
}
#[derive(Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
enum ReasonDto {
    Checkmate,
    Resignation,
}
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(deny_unknown_fields)]
struct SearchQuery {
    /// User ID, 1..128 UTF-8 bytes with no Unicode whitespace.
    participant: Option<String>,
    rule: Option<RuleDto>,
    winner: Option<PlayerDto>,
    reason: Option<ReasonDto>,
    /// Inclusive start in UTC microseconds. The interval must be at most 366 days.
    #[param(minimum = 0)]
    from: i64,
    /// Exclusive end in UTC microseconds, greater than from.
    #[param(minimum = 1)]
    to: i64,
    /// Inclusive minimum move ply, no greater than max_ply.
    #[param(minimum = 0, maximum = 9223372036854775807_u64)]
    min_ply: Option<u64>,
    /// Inclusive maximum move ply.
    #[param(minimum = 0, maximum = 9223372036854775807_u64)]
    max_ply: Option<u64>,
    #[param(minimum = 1, maximum = 100, default = 30)]
    limit: Option<u32>,
    /// Opaque cursor, at most 1024 bytes, bound to the same filters. Limit may change.
    cursor: Option<String>,
}
impl SearchQuery {
    fn parse(self) -> Result<Search, Error> {
        let participant = self
            .participant
            .map(UserId::new)
            .transpose()
            .map_err(|_| Error::InvalidQuery)?;
        let rule = self.rule.map(|RuleDto::Standard| Rule::Standard);
        let winner = self.winner.map(|p| match p {
            PlayerDto::Black => Player::Black,
            PlayerDto::White => Player::White,
        });
        let reason = self.reason.map(|r| match r {
            ReasonDto::Checkmate => FinishReason::Checkmate,
            ReasonDto::Resignation => FinishReason::Resignation,
        });
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
/// Search public completed records. Unknown or repeated query fields are rejected.
/// Handled responses have Cache-Control: no-store.
#[utoipa::path(get, path = "/game-records", params(SearchQuery), responses(
        (status = 200, description = "Page of public metadata", body = PageDto),
        (status = 400, description = "Invalid input", body = ErrorBody),
        (status = 422, description = "History or response exceeds bounds", body = ErrorBody),
        (status = 500, description = "Invalid stored record", body = ErrorBody),
        (status = 503, description = "Storage unavailable or request capacity exhausted", body = ErrorBody)
))]
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
/// Read public history. Missing, ongoing and hidden records are indistinguishable.
/// Handled responses have Cache-Control: no-store.
#[utoipa::path(get, path = "/game-records/{id}", params(
    ("id" = String, Path, description = "Game ID, 1..128 UTF-8 bytes with no Unicode whitespace")
), responses(
        (status = 200, description = "Public metadata and ordered history", body = DetailDto),
        (status = 404, description = "No public completed record", body = ErrorBody),
        (status = 400, description = "Invalid input", body = ErrorBody),
        (status = 422, description = "History or response exceeds bounds", body = ErrorBody),
        (status = 500, description = "Invalid stored record", body = ErrorBody),
        (status = 503, description = "Storage unavailable or request capacity exhausted", body = ErrorBody)
))]
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
        Error::InvalidQuery => (StatusCode::BAD_REQUEST, ErrorCode::InvalidQuery),
        Error::NotFound => (StatusCode::NOT_FOUND, ErrorCode::NotFound),
        Error::Corrupt => (StatusCode::INTERNAL_SERVER_ERROR, ErrorCode::InvalidRecord),
        Error::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, ErrorCode::Unavailable),
        Error::HistoryTooLong => (StatusCode::UNPROCESSABLE_ENTITY, ErrorCode::HistoryTooLong),
    };
    (status, axum::Json(ErrorBody { error: code })).into_response()
}
#[derive(Serialize, ToSchema)]
struct ErrorBody {
    error: ErrorCode,
}
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
enum ErrorCode {
    InvalidQuery,
    NotFound,
    InvalidRecord,
    Unavailable,
    HistoryTooLong,
}
fn player(player: Player) -> PlayerDto {
    if player == Player::Black {
        PlayerDto::Black
    } else {
        PlayerDto::White
    }
}
#[derive(Serialize, ToSchema)]
struct ResultDto {
    winner: PlayerDto,
    reason: ReasonDto,
}
#[derive(Serialize, ToSchema)]
struct MetadataDto {
    id: String,
    black: String,
    white: String,
    rule: RuleDto,
    result: ResultDto,
    /// UTC microseconds.
    finished_at: i64,
    #[schema(format = "uint64", minimum = 0, maximum = 18446744073709551615_u64)]
    final_seq: u64,
    #[schema(format = "uint64", minimum = 0, maximum = 18446744073709551615_u64)]
    final_move_ply: u64,
}
impl From<Metadata> for MetadataDto {
    fn from(m: Metadata) -> Self {
        Self {
            id: m.id.as_str().into(),
            black: m.black.as_str().into(),
            white: m.white.as_str().into(),
            rule: RuleDto::Standard,
            result: ResultDto {
                winner: player(m.result.winner),
                reason: if m.result.reason == FinishReason::Checkmate {
                    ReasonDto::Checkmate
                } else {
                    ReasonDto::Resignation
                },
            },
            finished_at: m.finished_at,
            final_seq: m.final_seq,
            final_move_ply: m.final_move_ply,
        }
    }
}
#[derive(Serialize, ToSchema)]
struct PageDto {
    items: Vec<MetadataDto>,
    #[schema(required = true)]
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
#[derive(Serialize, ToSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
enum ActionDto {
    Move { usi: String },
    Resign,
}
#[derive(Serialize, ToSchema)]
struct OperationDto {
    #[schema(format = "uint64", minimum = 0, maximum = 18446744073709551615_u64)]
    seq: u64,
    player: PlayerDto,
    #[serde(flatten)]
    action: ActionDto,
}
#[derive(Serialize, ToSchema)]
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

#[cfg(test)]
mod openapi_tests;
