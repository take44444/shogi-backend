use super::*;

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
