use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "生成されたgRPCクライアントをTCP経由で使い、GameRpc、アプリケーション、実際のPostgreSQLを通る。テスト専用の対局作成処理は事前準備として使う。",
        reason: "Matched中は対局を取得できず、両者の準備完了後だけPlayingになる。Matched・Playing状態のキャンセルは同じチケットを返す。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "rpc-boundary", scope: "生成されたgRPCクライアントをTCP経由で使い、GameRpc、アプリケーション、実際のPostgreSQLを通る。テスト専用の対局作成処理は事前準備として使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "未知・未指定のプールと0以下のハートビートはInvalidArgumentとなる。完全に同じ再試行はcreated=falseで変更のないチケットを返し、別の所有者にはNotFoundを返す。対局中・キャンセルの応答は対局の識別情報を保持する。" }],
    };
#[tokio::test]
async fn matchmaking_rpc_validates_requests_and_recovers_durable_owner_scoped_results() {
    let server = TestServer::start().await;
    let mut client = server.client().await;
    let channel = tonic::transport::Channel::from_shared(server.endpoint.clone())
        .unwrap()
        .connect()
        .await
        .unwrap();
    let mut raw = tonic::client::Grpc::new(channel);
    raw.ready().await.unwrap();
    let removed: Result<tonic::Response<proto::MatchmakingTicket>, _> = raw
        .unary(
            tonic::Request::new(proto::MatchmakingRequest {
                user_id: "one".into(),
                ticket_id: game_application::matchmaking::TicketId::new().to_string(),
            }),
            tonic::codegen::http::uri::PathAndQuery::from_static(
                "/shogi.v1.GameService/GetMatchmakingTicket",
            ),
            tonic_prost::ProstCodec::default(),
        )
        .await;
    assert_eq!(removed.unwrap_err().code(), Code::Unimplemented);
    let id = game_application::matchmaking::TicketId::new().to_string();
    let request = proto::JoinMatchmakingRequest {
        user_id: "one".into(),
        ticket_id: id.clone(),
        pool: proto::RatingPool::TenMinute30 as i32,
    };
    for pool in [0, 99] {
        let mut invalid = request.clone();
        invalid.pool = pool;
        assert_eq!(
            client.join_matchmaking(invalid).await.unwrap_err().code(),
            Code::InvalidArgument
        );
    }
    let accepted = client
        .join_matchmaking(request.clone())
        .await
        .unwrap()
        .into_inner();
    assert!(accepted.created);
    let retry = client
        .join_matchmaking(request.clone())
        .await
        .unwrap()
        .into_inner();
    assert!(!retry.created);
    assert_eq!(retry.ticket, accepted.ticket);
    let command = proto::MatchmakingRequest {
        user_id: "one".into(),
        ticket_id: id.clone(),
    };
    for sequence in [0, -1] {
        assert_eq!(
            client
                .heartbeat_matchmaking(proto::HeartbeatMatchmakingRequest {
                    user_id: "one".into(),
                    ticket_id: id.clone(),
                    sequence
                })
                .await
                .unwrap_err()
                .code(),
            Code::InvalidArgument
        );
    }
    assert_eq!(
        client
            .heartbeat_matchmaking(proto::HeartbeatMatchmakingRequest {
                user_id: "other".into(),
                ticket_id: id.clone(),
                sequence: 1,
            })
            .await
            .unwrap_err()
            .code(),
        Code::NotFound
    );
    assert_eq!(
        client
            .heartbeat_matchmaking(proto::HeartbeatMatchmakingRequest {
                user_id: "one".into(),
                ticket_id: game_application::matchmaking::TicketId::new().to_string(),
                sequence: 1,
            })
            .await
            .unwrap_err()
            .code(),
        Code::NotFound
    );
    assert!(
        client
            .get_matchmaking(proto::MatchmakingOwner {
                user_id: "other".into()
            })
            .await
            .unwrap()
            .into_inner()
            .ticket
            .is_none()
    );
    assert_eq!(
        client
            .ready_matchmaking(command.clone())
            .await
            .unwrap()
            .into_inner(),
        accepted.ticket.clone().unwrap()
    );
    for request in [
        proto::MatchmakingRequest {
            user_id: "other".into(),
            ticket_id: id.clone(),
        },
        proto::MatchmakingRequest {
            user_id: "one".into(),
            ticket_id: game_application::matchmaking::TicketId::new().to_string(),
        },
    ] {
        assert_eq!(
            client.ready_matchmaking(request).await.unwrap_err().code(),
            Code::NotFound
        );
    }
    assert_eq!(
        client
            .ready_matchmaking(proto::MatchmakingRequest {
                user_id: "one".into(),
                ticket_id: "invalid".into(),
            })
            .await
            .unwrap_err()
            .code(),
        Code::InvalidArgument
    );
    let other = game_application::matchmaking::TicketId::new().to_string();
    client
        .join_matchmaking(proto::JoinMatchmakingRequest {
            user_id: "two".into(),
            ticket_id: other.clone(),
            pool: request.pool,
        })
        .await
        .unwrap();
    let game = game_application::matchmaking::MatchmakingRepository::tick(
        server.store.as_ref(),
        game_application::rating::Pool::TenMinute30,
    )
    .await
    .unwrap()
    .unwrap();
    let ticket = client
        .heartbeat_matchmaking(proto::HeartbeatMatchmakingRequest {
            user_id: "one".into(),
            ticket_id: id.clone(),
            sequence: 1,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(ticket.state, "matched");
    assert_eq!(ticket.ready, Some(false));
    assert!(ticket.ready_deadline.is_some());
    assert_eq!(ticket.game_id.as_deref(), Some(game.as_str()));
    assert_eq!(
        client
            .cancel_matchmaking(command.clone())
            .await
            .unwrap()
            .into_inner(),
        ticket
    );
    assert_eq!(
        client
            .get_snapshot(proto::GetSnapshotRequest {
                game_id: game.as_str().into(),
            })
            .await
            .unwrap_err()
            .code(),
        Code::NotFound
    );
    let confirmed = client
        .ready_matchmaking(command.clone())
        .await
        .unwrap()
        .into_inner();
    assert_eq!(confirmed.state, "matched");
    assert_eq!(confirmed.ready, Some(true));
    assert_eq!(confirmed.ready_deadline, ticket.ready_deadline);
    assert_eq!(
        client
            .ready_matchmaking(command.clone())
            .await
            .unwrap()
            .into_inner(),
        confirmed
    );
    assert_eq!(
        client
            .get_snapshot(proto::GetSnapshotRequest {
                game_id: game.as_str().into(),
            })
            .await
            .unwrap_err()
            .code(),
        Code::NotFound
    );
    assert_eq!(
        client
            .heartbeat_matchmaking(proto::HeartbeatMatchmakingRequest {
                user_id: "one".into(),
                ticket_id: id.clone(),
                sequence: 2,
            })
            .await
            .unwrap()
            .into_inner(),
        confirmed
    );
    let playing = client
        .ready_matchmaking(proto::MatchmakingRequest {
            user_id: "two".into(),
            ticket_id: other,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(playing.state, "playing");
    assert_eq!(playing.game_id, ticket.game_id);
    assert!(playing.ready.is_none());
    assert!(playing.ready_deadline.is_none());
    let tracked = client
        .heartbeat_matchmaking(proto::HeartbeatMatchmakingRequest {
            user_id: "one".into(),
            ticket_id: id.clone(),
            sequence: 3,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(tracked.state, "playing");
    assert_eq!(tracked.game_id, playing.game_id);
    assert!(tracked.expires_at.is_none());
    assert!(tracked.heartbeat_sequence.is_none());
    assert!(tracked.ready_deadline.is_none());
    let snapshot = client
        .get_snapshot(proto::GetSnapshotRequest {
            game_id: game.as_str().into(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(snapshot.seq, 0);
    let ticket = client
        .heartbeat_matchmaking(proto::HeartbeatMatchmakingRequest {
            user_id: "one".into(),
            ticket_id: id.clone(),
            sequence: 4,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(ticket.state, "playing");
    assert_eq!(
        client
            .ready_matchmaking(command.clone())
            .await
            .unwrap()
            .into_inner(),
        ticket
    );
    assert_eq!(
        client
            .join_matchmaking(request)
            .await
            .unwrap()
            .into_inner()
            .ticket,
        Some(ticket.clone())
    );
    assert_eq!(
        client
            .cancel_matchmaking(command)
            .await
            .unwrap()
            .into_inner(),
        ticket
    );
    server.shutdown().await;
}
}
