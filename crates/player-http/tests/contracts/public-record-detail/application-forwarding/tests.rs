use super::*;
use crate::record_route_support::unused_ratings;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "player-http/frontend-get-game-records-id", relationship: crate::trace_support::Relationship::Asserts, scope: "公開 API で構築した実ルーター。repository の実装は準備コードで実 DB または fake を選択する。", reason: "公開ルーターの応答本文・ステータス・ヘッダーを検証する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["player-http"], fake: &[] }, crate::trace_support::Evidence { contract: "game-record-application/http-record-detail", relationship: crate::trace_support::Relationship::Exercises, scope: "公開 API で構築した実ルーター。repository の実装は準備コードで実 DB または fake を選択する。", reason: "実アプリケーションを通って PostgreSQL の読み取りへ到達する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-record-application"], fake: &[] }, crate::trace_support::Evidence { contract: "player-http/frontend-get-game-records", relationship: crate::trace_support::Relationship::Asserts, scope: "公開 API で構築した実ルーター。repository の実装は準備コードで実 DB または fake を選択する。", reason: "一覧と詳細の公開メタデータの一致と、公開検索条件を検証する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["player-http"], fake: &[] }] };
#[tokio::test]
async fn anonymous_list_is_metadata_only_and_detail_contains_only_public_ordered_actions() {
    let database = pg_test_support::TestDatabase::new().await;
    let store = Arc::new(SeaOrmStore::connect(database.url()).await.unwrap());
    let games = pg_test_support::GameFixture::new(store, database.url());
    let id = WriteId::new("game").unwrap();
    games
        .seed(
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
        .seed(
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
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "player-http/frontend-get-game-records-id", relationship: crate::trace_support::Relationship::Asserts, scope: "公開ルーターと実 GameRecords、fake RecordRepository。詳細取得だけを実行する。", reason: "詳細取得で履歴上限・不正入力・保存失敗に対応する公開エラーとヘッダーを検査する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["player-http", "game-record-application"], fake: &["game-record-persistence"] }, crate::trace_support::Evidence { contract: "game-record-application/http-record-detail", relationship: crate::trace_support::Relationship::Exercises, scope: "公開ルーターと実 GameRecords、fake RecordRepository。詳細取得だけを実行する。", reason: "実 application を通って失敗を返す fake RecordRepository に到達する。PostgreSQL は使用しない。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["player-http", "game-record-application"], fake: &["game-record-persistence"] }] };
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
}
