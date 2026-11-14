use crate::{AuthError, Gateway, one_header};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use game_application::UserId;
use game_application::matchmaking::{HeartbeatSequence, TicketId};
use game_application::rating::Pool;
use game_grpc::proto;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use tonic::{Code, Status};
use utoipa::{OpenApi, ToSchema};

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct Join {
    pool: String,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct Heartbeat {
    sequence: i64,
}
#[derive(Serialize, ToSchema)]
struct Ticket {
    ticket_id: String,
    pool: Option<String>,
    state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    enqueued_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    heartbeat_sequence: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    game_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    side: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
}
#[derive(Serialize, ToSchema)]
struct Current {
    state: String,
    ticket: Option<Ticket>,
}
impl TryFrom<proto::MatchmakingTicket> for Ticket {
    type Error = Status;
    fn try_from(t: proto::MatchmakingTicket) -> Result<Self, Self::Error> {
        let pool = t
            .pool
            .map(|p| match proto::RatingPool::try_from(p) {
                Ok(proto::RatingPool::TenMinute30) => Ok("ten_minute_30".into()),
                Ok(proto::RatingPool::Byoyomi30) => Ok("byoyomi_30".into()),
                _ => Err(Status::data_loss("invalid pool")),
            })
            .transpose()?;
        let side = t
            .side
            .map(|p| match proto::Player::try_from(p) {
                Ok(proto::Player::Black) => Ok("black".into()),
                Ok(proto::Player::White) => Ok("white".into()),
                _ => Err(Status::data_loss("invalid side")),
            })
            .transpose()?;
        if !matches!(
            t.state.as_str(),
            "waiting" | "playing" | "finished" | "cancelled" | "expired" | "refused"
        ) {
            return Err(Status::data_loss("invalid ticket state"));
        }
        Ok(Self {
            ticket_id: t.ticket_id,
            pool,
            state: t.state,
            enqueued_at: t.enqueued_at,
            expires_at: t.expires_at,
            heartbeat_sequence: t.heartbeat_sequence,
            game_id: t.game_id,
            side,
            code: t.refusal_reason,
        })
    }
}
#[derive(Serialize, ToSchema)]
struct ErrorBody {
    code: String,
}
#[derive(Serialize, ToSchema)]
#[serde(untagged)]
enum TicketResponse {
    Ticket(Ticket),
    Error(ErrorBody),
}
pub(crate) fn router() -> Router<Arc<Gateway>> {
    Router::new()
        .route("/matchmaking/me", get(current))
        .route(
            "/matchmaking/tickets/{ticket_id}",
            get(ticket).put(join).delete(cancel),
        )
        .route(
            "/matchmaking/tickets/{ticket_id}/heartbeat",
            post(heartbeat),
        )
        .layer(middleware::map_response(no_store))
}
async fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
async fn authenticate(
    gateway: &Gateway,
    headers: &HeaderMap,
    mutation: bool,
) -> Result<UserId, StatusCode> {
    if *gateway.shutdown.borrow() {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    let expected_host = gateway.origin.split_once("://").map(|(_, host)| host);
    if one_header(headers, header::HOST) != expected_host
        || (mutation && one_header(headers, header::ORIGIN) != Some(gateway.origin.as_str()))
        || (headers.contains_key(header::ORIGIN)
            && one_header(headers, header::ORIGIN) != Some(gateway.origin.as_str()))
        || (headers.contains_key("sec-fetch-site")
            && one_header(headers, "sec-fetch-site".parse().unwrap()) != Some("same-origin"))
    {
        return Err(StatusCode::FORBIDDEN);
    }
    let cookie = gateway
        .auth
        .cookie(headers)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    gateway
        .auth
        .authenticate(&cookie)
        .await
        .map_err(|e| match e {
            AuthError::Unauthorized => StatusCode::UNAUTHORIZED,
            AuthError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        })
}
fn rpc_error(status: Status) -> Response {
    let (code, name) = match status.code() {
        Code::NotFound => (StatusCode::NOT_FOUND, "not_found"),
        Code::InvalidArgument => (StatusCode::BAD_REQUEST, "invalid_input"),
        Code::AlreadyExists => (StatusCode::CONFLICT, "pool_conflict"),
        _ => (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
    };
    (
        code,
        Json(TicketResponse::Error(ErrorBody { code: name.into() })),
    )
        .into_response()
}
async fn rpc<T>(
    request: impl std::future::Future<Output = Result<tonic::Response<T>, Status>>,
) -> Result<T, Status> {
    tokio::time::timeout(Duration::from_secs(3), request)
        .await
        .map_err(|_| Status::deadline_exceeded("matchmaking request timed out"))?
        .map(tonic::Response::into_inner)
}
fn ticket_response(
    raw: proto::MatchmakingTicket,
    created: bool,
    cancel: bool,
    join: bool,
) -> Response {
    let mut ticket = match Ticket::try_from(raw) {
        Ok(t) => t,
        Err(e) => return rpc_error(e),
    };
    let status = if cancel && matches!(ticket.state.as_str(), "playing" | "finished") {
        ticket.code = Some("already_matched".into());
        StatusCode::CONFLICT
    } else if join && ticket.state == "refused" {
        StatusCode::CONFLICT
    } else if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    (status, Json(TicketResponse::Ticket(ticket))).into_response()
}

#[utoipa::path(put,path="/matchmaking/tickets/{ticket_id}",params(("ticket_id"=String,Path,description="Owner-scoped UUID, preserved on retries")),request_body=Join,responses((status=201,body=Ticket,description="New waiting request"),(status=200,body=Ticket,description="Stored retry result"),(status=409,body=TicketResponse,description="Durable refusal or pool conflict"),(status=400,description="Malformed input"),(status=401,description="Authentication required"),(status=403,description="Origin or Host rejected"),(status=503,description="Retry the same UUID or query its state")))]
async fn join(
    State(gateway): State<Arc<Gateway>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<Join>, JsonRejection>,
) -> Response {
    let user = match authenticate(&gateway, &headers, true).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };
    let Ok(Json(body)) = body else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let (Ok(id), Ok(pool)) = (TicketId::parse(&id), Pool::parse(&body.pool)) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let pool = match pool {
        Pool::TenMinute30 => proto::RatingPool::TenMinute30,
        Pool::Byoyomi30 => proto::RatingPool::Byoyomi30,
    } as i32;
    match rpc(gateway
        .games
        .client()
        .join_matchmaking(proto::JoinMatchmakingRequest {
            user_id: user.as_str().into(),
            ticket_id: id.to_string(),
            pool,
        }))
    .await
    {
        Ok(response) => match response.ticket {
            Some(t) => ticket_response(t, response.created, false, true),
            None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
        },
        Err(e) => rpc_error(e),
    }
}
#[utoipa::path(get,path="/matchmaking/me",responses((status=200,body=Current,description="Idle, waiting or playing; GET does not renew the ticket"),(status=401,description="Authentication required"),(status=503,description="Storage unavailable")))]
async fn current(State(gateway): State<Arc<Gateway>>, headers: HeaderMap) -> Response {
    let user = match authenticate(&gateway, &headers, false).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };
    match rpc(gateway
        .games
        .client()
        .get_matchmaking(proto::MatchmakingOwner {
            user_id: user.as_str().into(),
        }))
    .await
    {
        Ok(response) => {
            let ticket = match response.ticket.map(Ticket::try_from).transpose() {
                Ok(t) => t,
                Err(e) => return rpc_error(e),
            };
            let state = ticket
                .as_ref()
                .map_or("idle", |t| t.state.as_str())
                .to_owned();
            Json(Current { state, ticket }).into_response()
        }
        Err(e) => rpc_error(e),
    }
}
#[utoipa::path(get,path="/matchmaking/tickets/{ticket_id}",params(("ticket_id"=String,Path)),responses((status=200,body=Ticket,description="Stored owner-scoped request"),(status=404,description="No ticket for this owner"),(status=401,description="Authentication required"),(status=503,description="Storage unavailable")))]
async fn ticket(
    State(gateway): State<Arc<Gateway>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    read_or_cancel(gateway, id, headers, false).await
}
#[utoipa::path(delete,path="/matchmaking/tickets/{ticket_id}",params(("ticket_id"=String,Path)),responses((status=200,body=Ticket,description="Cancelled or terminal ticket"),(status=409,body=Ticket,description="Already matched, including committed game ID"),(status=401,description="Authentication required"),(status=403,description="Origin or Host rejected"),(status=503,description="Retry the same UUID or query its state")))]
async fn cancel(
    State(gateway): State<Arc<Gateway>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    read_or_cancel(gateway, id, headers, true).await
}
async fn read_or_cancel(
    gateway: Arc<Gateway>,
    id: String,
    headers: HeaderMap,
    cancel: bool,
) -> Response {
    let user = match authenticate(&gateway, &headers, cancel).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };
    let Ok(id) = TicketId::parse(&id) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let request = proto::MatchmakingRequest {
        user_id: user.as_str().into(),
        ticket_id: id.to_string(),
    };
    let mut client = gateway.games.client();
    let result = if cancel {
        rpc(client.cancel_matchmaking(request)).await
    } else {
        rpc(client.get_matchmaking_ticket(request)).await
    };
    match result {
        Ok(t) => ticket_response(t, false, cancel, false),
        Err(e) => rpc_error(e),
    }
}
#[utoipa::path(post,path="/matchmaking/tickets/{ticket_id}/heartbeat",params(("ticket_id"=String,Path)),request_body=Heartbeat,responses((status=200,body=Ticket,description="Current ticket; only a newer sequence extends live waiting"),(status=400,description="Sequence must be in 1 through i64::MAX"),(status=401,description="Authentication required"),(status=403,description="Origin or Host rejected"),(status=404,description="No ticket for this owner"),(status=503,description="Retry with the same sequence")))]
async fn heartbeat(
    State(gateway): State<Arc<Gateway>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<Heartbeat>, JsonRejection>,
) -> Response {
    let user = match authenticate(&gateway, &headers, true).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };
    let Ok(Json(body)) = body else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let (Ok(id), Some(sequence)) = (TicketId::parse(&id), HeartbeatSequence::new(body.sequence))
    else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    match rpc(gateway
        .games
        .client()
        .heartbeat_matchmaking(proto::HeartbeatMatchmakingRequest {
            user_id: user.as_str().into(),
            ticket_id: id.to_string(),
            sequence: sequence.get(),
        }))
    .await
    {
        Ok(t) => ticket_response(t, false, false, false),
        Err(e) => rpc_error(e),
    }
}
#[derive(OpenApi)]
#[openapi(
    paths(join, current, ticket, cancel, heartbeat),
    components(schemas(Join, Heartbeat, Ticket, Current, ErrorBody, TicketResponse)),
    security(("session" = []), ("development_session" = [])),
    modifiers(&CookieSecurity)
)]
struct Api;
pub(crate) fn openapi() -> utoipa::openapi::OpenApi {
    Api::openapi()
}

struct CookieSecurity;
impl utoipa::Modify for CookieSecurity {
    fn modify(&self, api: &mut utoipa::openapi::OpenApi) {
        use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
        api.info = utoipa::openapi::Info::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        let components = api.components.get_or_insert_with(Default::default);
        for (scheme, cookie) in [
            ("session", "__Host-user_session"),
            ("development_session", "user_session"),
        ] {
            components.add_security_scheme(
                scheme,
                SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new(cookie))),
            );
        }
    }
}
