use super::*;
use axum::{body::Body, http::Request};
use game_application::{GameId as WriteId, GameService, MakeMove, Participants, RequestId, Resign};
use game_persistence::SeaOrmStore;
use game_record_persistence::PostgresGameRecords;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

async fn get(app: Router, path: &str) -> (StatusCode, Value) {
    let response = app
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}
#[tokio::test]
async fn anonymous_list_is_metadata_only_and_detail_contains_only_public_ordered_actions() {
    let database = pg_test_support::TestDatabase::new().await;
    let store = Arc::new(SeaOrmStore::connect(database.url()).await.unwrap());
    let games = GameService::new(store);
    let id = WriteId::new("game").unwrap();
    games
        .create(
            id.clone(),
            Participants::new(UserId::new("black").unwrap(), UserId::new("white").unwrap())
                .unwrap(),
        )
        .await
        .unwrap();
    games
        .make_move(
            &id,
            MakeMove {
                request_id: RequestId::new("private-retry-id").unwrap(),
                expected_seq: 0,
                user_id: UserId::new("black").unwrap(),
                movement: "7g7f".parse().unwrap(),
            },
        )
        .await
        .unwrap();
    games
        .resign(
            &id,
            Resign {
                request_id: RequestId::new("private-finish-id").unwrap(),
                expected_seq: 1,
                user_id: UserId::new("white").unwrap(),
            },
        )
        .await
        .unwrap();
    games
        .create(
            WriteId::new("ongoing").unwrap(),
            Participants::new(UserId::new("black").unwrap(), UserId::new("white").unwrap())
                .unwrap(),
        )
        .await
        .unwrap();
    let reader = Arc::new(
        PostgresGameRecords::connect(&database.reader_url().await)
            .await
            .unwrap(),
    );
    let app = router(
        GameRecords::new(reader.clone()),
        UserRatings::new(reader.clone()),
    );
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as i64;
    let query = format!(
        "/game-records?from={}&to={}&participant=white&rule=standard&winner=black&reason=resignation&min_ply=1&max_ply=1&limit=1",
        now - 1_000_000,
        now + 1_000_000
    );
    let (status, list) = get(app.clone(), &query).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    let item = &list["items"][0];
    assert_eq!(item["id"], "game");
    assert_eq!(item["final_seq"], 2);
    assert_eq!(item["final_move_ply"], 1);
    let mut keys = item
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "black",
            "final_move_ply",
            "final_seq",
            "finished_at",
            "id",
            "result",
            "rule",
            "white"
        ]
    );
    let (status, detail) = get(app.clone(), "/game-records/game").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["metadata"], *item);
    assert_eq!(
        detail["history"],
        serde_json::json!([
            {"seq":1,"player":"black","action":"move","usi":"7g7f"},
            {"seq":2,"player":"white","action":"resign"}
        ])
    );
    let encoded = serde_json::to_string(&detail).unwrap();
    for secret in [
        "request_id",
        "expected_seq",
        "acceptance",
        "receipt",
        "in_check",
        "private-retry-id",
        "private-finish-id",
        "outbox",
    ] {
        assert!(!encoded.contains(secret));
    }
    let missing = get(app.clone(), "/game-records/missing").await;
    let ongoing = get(app.clone(), "/game-records/ongoing").await;
    assert_eq!(
        missing,
        (
            StatusCode::NOT_FOUND,
            serde_json::json!({"error":"not_found"})
        )
    );
    assert_eq!(ongoing, missing);
    reader.close().await;
    assert_eq!(
        get(app, &query).await,
        (
            StatusCode::SERVICE_UNAVAILABLE,
            serde_json::json!({"error":"unavailable"})
        )
    );
}
#[tokio::test]
async fn query_boundary_rejects_bad_filters_bounds_and_cursors_before_storage() {
    let repository = Arc::new(Failing(game_record_application::Error::Corrupt));
    let app = router(GameRecords::new(repository), unused_ratings());
    for path in [
        "/game-records",
        "/game-records?from=0&to=0",
        "/game-records?from=-1&to=2",
        "/game-records?from=0&to=31622400000001",
        "/game-records?from=0&to=100&limit=0",
        "/game-records?from=0&to=100&limit=101",
        "/game-records?from=0&to=100&min_ply=2&max_ply=1",
        "/game-records?from=0&to=100&min_ply=18446744073709551615",
        "/game-records?from=0&to=100&participant=bad%20id",
        "/game-records?from=0&to=100&winner=alice",
        "/game-records?from=0&to=100&rule=other",
        "/game-records?from=0&to=100&reason=other",
        "/game-records?from=0&to=100&cursor=garbage",
        "/game-records?from=0&to=100&unknown=1",
        "/game-records?from=0&to=100&limit=1&limit=2",
        "/game-records/bad%20id",
    ] {
        assert_eq!(
            get(app.clone(), path).await,
            (
                StatusCode::BAD_REQUEST,
                serde_json::json!({"error":"invalid_query"})
            ),
            "{path}"
        );
    }
    assert_eq!(
        get(app, "/game-records?from=0&to=100").await.0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
}
struct Failing(Error);
#[async_trait::async_trait]
impl game_record_application::RecordRepository for Failing {
    async fn search(&self, _: &Search) -> Result<Page, Error> {
        Err(self.0)
    }
    async fn detail(&self, _: &GameId) -> Result<Detail, Error> {
        Err(self.0)
    }
}
#[tokio::test]
async fn bounded_detail_and_storage_errors_are_generic() {
    for (failure, status, code) in [
        (
            Error::Corrupt,
            StatusCode::INTERNAL_SERVER_ERROR,
            "invalid_record",
        ),
        (
            Error::HistoryTooLong,
            StatusCode::UNPROCESSABLE_ENTITY,
            "history_too_long",
        ),
        (
            Error::Unavailable,
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
        ),
    ] {
        let app = router(
            GameRecords::new(Arc::new(Failing(failure))),
            unused_ratings(),
        );
        assert_eq!(
            get(app, "/game-records/game").await,
            (status, serde_json::json!({"error":code}))
        );
    }
}
