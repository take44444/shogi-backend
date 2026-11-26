use super::*;
use crate::record_route_support::unused_ratings;
use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use tokio::sync::{Semaphore, mpsc};
use tower::ServiceExt;

struct Blocked {
    entered: mpsc::UnboundedSender<u32>,
    release: Semaphore,
}

#[async_trait::async_trait]
impl game_record_application::RecordRepository for Blocked {
    async fn search(&self, search: &Search) -> Result<Page, Error> {
        self.entered.send(search.limit()).unwrap();
        self.release.acquire().await.unwrap().forget();
        Ok(Page {
            items: vec![],
            next_cursor: None,
        })
    }

    async fn detail(&self, _: &GameId) -> Result<Detail, Error> {
        unreachable!()
    }
}

async fn request(app: Router, path: &str) -> Response {
    app.oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn sixteen_active_requests_admit_no_seventeenth_and_release_capacity() {
    let (entered, mut arrivals) = mpsc::unbounded_channel();
    let repository = Arc::new(Blocked {
        entered,
        release: Semaphore::new(0),
    });
    let app = router(GameRecords::new(repository.clone()), unused_ratings());
    let mut requests = Vec::new();
    for _ in 0..16 {
        let app = app.clone();
        requests.push(tokio::spawn(async move {
            request(app, "/game-records?from=0&to=31622400000000").await
        }));
    }
    for _ in 0..16 {
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(5), arrivals.recv())
                .await
                .unwrap(),
            Some(30)
        );
    }
    let rejected = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        request(app.clone(), "/game-records?from=0&to=100"),
    )
    .await
    .expect("the seventeenth request must be rejected without waiting for storage");
    assert_eq!(rejected.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(rejected.headers()[header::CACHE_CONTROL], "no-store");
    assert!(arrivals.try_recv().is_err());
    repository.release.add_permits(16);
    for task in requests {
        assert_eq!(task.await.unwrap().status(), StatusCode::OK);
    }
    let last =
        tokio::spawn(async move { request(app, "/game-records?from=0&to=100&limit=100").await });
    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(5), arrivals.recv())
            .await
            .unwrap(),
        Some(100)
    );
    repository.release.add_permits(1);
    assert_eq!(last.await.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn serialized_response_accepts_one_mib_and_rejects_the_next_byte() {
    let response = respond(Ok("x".repeat(1_048_574)));
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(body.len(), 1_048_576);

    let response = respond(Ok("x".repeat(1_048_575)));
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        serde_json::json!({"error":"history_too_long"})
    );
}

#[tokio::test]
async fn timeout_search_and_public_history_keep_the_distinct_finish_reason() {
    struct Timeouts;
    fn metadata() -> Metadata {
        Metadata::new(
            GameId::new("timeout").unwrap(),
            UserId::new("black").unwrap(),
            UserId::new("white").unwrap(),
            Rule::Standard,
            game_record_application::GameResult {
                winner: Player::White,
                reason: FinishReason::Timeout,
            },
            game_record_application::PreparedFinishTime::try_from(10_i64).unwrap(),
            game_record_application::OperationSequence::try_from(1_u64).unwrap(),
            game_record_application::MovePly::try_from(0_u64).unwrap(),
        )
        .unwrap()
    }
    #[async_trait::async_trait]
    impl game_record_application::RecordRepository for Timeouts {
        async fn search(&self, search: &Search) -> Result<Page, Error> {
            assert_eq!(search.filters().reason, Some(FinishReason::Timeout));
            Ok(Page {
                items: vec![metadata()],
                next_cursor: None,
            })
        }
        async fn detail(&self, _: &GameId) -> Result<Detail, Error> {
            Ok(Detail {
                metadata: metadata(),
                history: vec![game_record_application::PublicOperation {
                    seq: game_record_application::OperationSequence::try_from(1_u64).unwrap(),
                    player: Player::Black,
                    action: PublicAction::Timeout,
                }],
            })
        }
    }
    let app = router(GameRecords::new(Arc::new(Timeouts)), unused_ratings());
    let response = request(app.clone(), "/game-records?from=0&to=100&reason=timeout").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["items"][0]["result"]["reason"], "timeout");
    let body = request(app, "/game-records/timeout")
        .await
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        json["history"][0],
        serde_json::json!({"seq":1,"player":"black","action":"timeout"})
    );
}
