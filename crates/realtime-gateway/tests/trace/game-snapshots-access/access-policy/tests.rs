use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。",
        reason: "非公開対局の参加者以外と存在しない対局へのアクセス確認RPCはNotFoundとなる。参加者のアクセスは成功し、公開範囲の変更後は既存の観戦者の接続を閉じる。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "game-snapshots-access", contract: "viewer-read", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "非公開対局の参加者以外と存在しない対局の閲覧者用スナップショットはNotFoundとなる。参加者のスナップショット取得は成功する。" }, crate::trace_support::Evidence { feature: "game-snapshots-access", contract: "grpc-viewer", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "生成されたTCPのRPCを実際のゲートウェイ用テストサービスに通し、非公開・不存在のエラーと参加者の成功を検証する。" }, crate::trace_support::Evidence { feature: "game-snapshots-access", contract: "durable-read", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。", relationship: crate::trace_support::Relationship::Exercises, reason: "閲覧者用の読み取り処理で、非公開対局と参加者のアクセス確認の経路を通す。" }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "upgrade-permits", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "非公開対局の参加者以外と存在しない対局には、区別できない404のアップグレード応答ステータスと本文を返す。" }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "command-input", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "偽のuser_idはinvalid_inputとなり、連番は0のままとなる。観戦者の着手・投了はnot_participantとなる。" }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "command-response", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "参加者以外の着手・投了はnot_participantとなる。" }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "live-checks", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "観戦者は対局を変更できず、参加者の識別情報を指定することもできない。" }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "outgoing", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "既存の観戦者への公開を取り消すと、次のイベントで接続を閉じる。" }],
    };
#[tokio::test]
async fn private_games_are_hidden_and_spectators_cannot_mutate_or_spoof_a_participant() {
    let mut services = Services::start().await;
    let mut viewer = services.connect(&services.viewer).await;
    initial(&mut viewer, &services.game, "0").await;
    send(&mut viewer, movement("outsider", "0", "7g7f")).await;
    assert_eq!(receive(&mut viewer).await["code"], "not_participant");
    send(
        &mut viewer,
        json!({"type":"resign", "request_id":"outsider-resign", "expected_seq":"0"}),
    )
    .await;
    assert_eq!(receive(&mut viewer).await["code"], "not_participant");
    let mut forged = movement("forged", "0", "7g7f");
    forged["user_id"] = services.black.id.clone().into();
    send(&mut viewer, forged).await;
    assert_eq!(receive(&mut viewer).await["code"], "invalid_input");
    assert_eq!(services.snapshot().await.seq, 0);
    services.visibility("private").await;
    let denied = proto::ViewerRequest {
        game_id: services.game.clone(),
        viewer_user_id: services.viewer.id.clone(),
    };
    let missing = proto::ViewerRequest {
        game_id: uuid::Uuid::new_v4().to_string(),
        viewer_user_id: services.viewer.id.clone(),
    };
    for request in [denied, missing] {
        assert_eq!(
            services
                .client
                .get_snapshot_for_viewer(request.clone())
                .await
                .unwrap_err()
                .code(),
            tonic::Code::NotFound
        );
        assert_eq!(
            services
                .client
                .check_view_access(request)
                .await
                .unwrap_err()
                .code(),
            tonic::Code::NotFound
        );
    }
    let participant = proto::ViewerRequest {
        game_id: services.game.clone(),
        viewer_user_id: services.black.id.clone(),
    };
    assert_eq!(
        services
            .client
            .get_snapshot_for_viewer(participant.clone())
            .await
            .unwrap()
            .into_inner()
            .seq,
        0
    );
    services
        .client
        .check_view_access(participant)
        .await
        .unwrap();
    let hidden = rejected(services.request(&services.viewer)).await;
    let mut absent = services.request(&services.viewer);
    *absent.uri_mut() = format!(
        "{}/ws/games/{}",
        services.origin.replacen("http", "ws", 1),
        uuid::Uuid::new_v4()
    )
    .parse()
    .unwrap();
    assert_eq!(hidden, rejected(absent).await);
    assert_eq!(hidden.0, StatusCode::NOT_FOUND);
    let mut black = services.connect(&services.black).await;
    initial(&mut black, &services.game, "0").await;
    send(&mut black, movement("participant", "0", "7g7f")).await;
    assert_eq!(receive(&mut black).await["type"], "accepted");
    services.publish().await;
    closed(&mut viewer).await;
    services.stop().await;
}
}
