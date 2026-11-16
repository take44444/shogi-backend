use super::*;
use crate::record_route_support::unused_ratings;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実 Axum ルーター、アプリケーションサービス、PostgreSQL の読み取り処理。公開用シリアライズと、対局が存在しない場合・進行中の場合の動作を検証する。",
        reason: "実ルーターがこのメソッドを通り、PostgreSQL の読み取り処理に到達する。",
        relationship: crate::trace_support::Relationship::Exercises,
        also: &[crate::trace_support::Evidence { feature: "public-record-detail", contract: "http-boundary", scope: "実 Axum ルーター、アプリケーションサービス、PostgreSQL の読み取り処理。公開用シリアライズと、対局が存在しない場合・進行中の場合の動作を検証する。", relationship: crate::trace_support::Relationship::Asserts, reason: "実データベースで存在しない対局と進行中の対局が同じ 404 と本文を返し、詳細取得の成功応答に no-store が付くことを確認する。" }, crate::trace_support::Evidence { feature: "public-record-detail", contract: "public-dto", scope: "実 Axum ルーター、アプリケーションサービス、PostgreSQL の読み取り処理。公開用シリアライズと、対局が存在しない場合・進行中の場合の動作を検証する。", relationship: crate::trace_support::Relationship::Asserts, reason: "メタデータのキー集合の完全一致、一覧のメタデータとの一致、順序付きの二つの操作を検証する。シリアライズした詳細に、列挙した内部情報の部分文字列が含まれないことも確認する。" }, crate::trace_support::Evidence { feature: "public-record-search", contract: "query-boundary", scope: "実 Axum ルーター、アプリケーションサービス、PostgreSQL の読み取り処理。公開対局サービスの TCP リスナーは使わない。", relationship: crate::trace_support::Relationship::Asserts, reason: "実ルーターと PostgreSQL の読み取り処理が、参加者・ルール・勝者・終了理由・手数を組み合わせたクエリーに一致する一件の対局を返すことを確認する。" }, crate::trace_support::Evidence { feature: "public-record-search", contract: "application-forwarding", scope: "実 Axum ルーター、アプリケーションサービス、PostgreSQL の読み取り処理。公開対局サービスの TCP リスナーは使わない。", relationship: crate::trace_support::Relationship::Exercises, reason: "実ルーターがこのアプリケーションメソッドを通り、実 PostgreSQL の読み取り処理に到達する。" }, crate::trace_support::Evidence { feature: "public-record-search", contract: "public-keysets", scope: "実 Axum ルーター、アプリケーションサービス、PostgreSQL の読み取り処理。公開対局サービスの TCP リスナーは使わない。", relationship: crate::trace_support::Relationship::Asserts, reason: "認証なしの実ルーターが公開メタデータだけを返し、読み取り処理を閉じると利用不可となることを確認する。" }],
    };
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

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "リポジトリの失敗をテスト用の代替実装で再現する、プロセス内のルーター。データベースは使わない。",
        reason: "テスト用リポジトリのエラーが、HTTP 応答へ変換される前にこのメソッドを通る。",
        relationship: crate::trace_support::Relationship::Exercises,
        also: &[crate::trace_support::Evidence { feature: "public-record-detail", contract: "http-boundary", scope: "リポジトリの失敗をテスト用の代替実装で再現する、プロセス内のルーター。データベースは使わない。", relationship: crate::trace_support::Relationship::Asserts, reason: "テスト用リポジトリの破損・サイズ超過・利用不可の失敗が 500・422・503 となり、固定の JSON と no-store を返すことを確認する。" }],
    };
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
