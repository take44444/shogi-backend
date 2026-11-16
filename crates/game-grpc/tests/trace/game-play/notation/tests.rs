use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "生成されたgRPCクライアントをTCP経由で使い、GameRpc、アプリケーション、実際のPostgreSQLを通る。テスト専用の対局作成処理は事前準備として使う。",
        reason: "TCP経由の不正なUSIはInvalidArgumentとなり、スナップショットは変化しない。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "game-play", contract: "rpc-move", scope: "生成されたgRPCクライアントをTCP経由で使い、GameRpc、アプリケーション、実際のPostgreSQLを通る。テスト専用の対局作成処理は事前準備として使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "不正なID、連番の欠落、不正なUSIはInvalidArgumentとなり、スナップショットは変化しない。" }, crate::trace_support::Evidence { feature: "game-play", contract: "rpc-resign", scope: "生成されたgRPCクライアントをTCP経由で使い、GameRpc、アプリケーション、実際のPostgreSQLを通る。テスト専用の対局作成処理は事前準備として使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "不正な投了フィールドを、状態を変更せずに拒否する。" }],
    };
#[tokio::test]
async fn malformed_wire_values_are_invalid_arguments_without_changes() {
    let server = TestServer::start().await;
    let mut client = server.client().await;
    let initial = create(&mut client, "validation").await;
    for id in ["".to_owned(), "white space".into(), "x".repeat(129)] {
        assert_eq!(
            client
                .get_snapshot(proto::GetSnapshotRequest { game_id: id })
                .await
                .unwrap_err()
                .code(),
            Code::InvalidArgument
        );
    }
    let valid = command("validation", "correctable", 0, proto::Player::Black, "7g7f");
    let mut malformed = Vec::new();
    let mut missing = valid.clone();
    missing.expected_seq = None;
    malformed.push(missing);
    let mut missing_id = valid.clone();
    missing_id.game_id.clear();
    malformed.push(missing_id);
    let mut bad_id = valid.clone();
    bad_id.request_id = "\u{2003}".into();
    malformed.push(bad_id);
    for value in ["", "bad id", "\u{2003}"] {
        let mut bad_player = valid.clone();
        bad_player.user_id = value.into();
        malformed.push(bad_player);
    }
    for usi in ["", "7g7f+garbage", "resign", "P*0a", "7g7f\n"] {
        let mut bad_usi = valid.clone();
        bad_usi.usi = usi.into();
        malformed.push(bad_usi);
    }
    for request in malformed {
        assert_eq!(
            client.make_move(request).await.unwrap_err().code(),
            Code::InvalidArgument
        );
        assert_eq!(snapshot(&mut client, "validation").await, initial);
    }
    let valid_resign = resign("validation", "correctable", 0, proto::Player::White);
    let mut missing = valid_resign.clone();
    missing.expected_seq = None;
    let mut bad_request_id = valid_resign.clone();
    bad_request_id.request_id.clear();
    let mut bad_game_id = valid_resign.clone();
    bad_game_id.game_id.clear();
    let mut unspecified = valid_resign.clone();
    unspecified.user_id.clear();
    let mut unknown = valid_resign;
    unknown.user_id = "bad id".into();
    for request in [missing, bad_request_id, bad_game_id, unspecified, unknown] {
        assert_eq!(
            client.resign(request).await.unwrap_err().code(),
            Code::InvalidArgument
        );
        assert_eq!(snapshot(&mut client, "validation").await, initial);
    }
    assert_eq!(client.make_move(valid).await.unwrap().into_inner().seq, 1);
    drop(client);
    server.shutdown().await;
}
}
