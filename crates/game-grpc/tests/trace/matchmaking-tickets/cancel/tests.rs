use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Generated gRPC client over TCP, GameRpc, application and real PostgreSQL; test-only game seeding is fixture setup.",
        reason: "Cancel of an already Playing ticket equals its prior ticket.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "rpc-boundary", scope: "Generated gRPC client over TCP, GameRpc, application and real PostgreSQL; test-only game seeding is fixture setup.", relationship: crate::trace_support::Relationship::Asserts, reason: "Unknown/unspecified pool and nonpositive heartbeat are InvalidArgument, exact retry returns unchanged ticket with created=false, foreign owner is NotFound and playing/cancel responses preserve game identity." }],
    };
#[tokio::test]
async fn matchmaking_rpc_validates_requests_and_recovers_durable_owner_scoped_results() {
    let server = TestServer::start().await;
    let mut client = server.client().await;
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
    let lookup = proto::MatchmakingRequest {
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
            .get_matchmaking_ticket(proto::MatchmakingRequest {
                user_id: "other".into(),
                ticket_id: id.clone()
            })
            .await
            .unwrap_err()
            .code(),
        Code::NotFound
    );
    let other = game_application::matchmaking::TicketId::new().to_string();
    client
        .join_matchmaking(proto::JoinMatchmakingRequest {
            user_id: "two".into(),
            ticket_id: other,
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
        .get_matchmaking_ticket(lookup.clone())
        .await
        .unwrap()
        .into_inner();
    assert_eq!(ticket.state, "playing");
    assert_eq!(ticket.game_id.as_deref(), Some(game.as_str()));
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
            .cancel_matchmaking(lookup)
            .await
            .unwrap()
            .into_inner(),
        ticket
    );
    server.shutdown().await;
}
}
