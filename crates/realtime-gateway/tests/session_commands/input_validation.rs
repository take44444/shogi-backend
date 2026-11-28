use super::*;

#[tokio::test]
async fn oversized_frames_and_input_rate_close_without_committing_commands() {
    let mut services = Services::with_limits(Limits {
        input_bytes: 128,
        messages_per_second: 1,
        ..Limits::default()
    })
    .await;
    let mut oversized = services.connect(&services.black).await;
    initial(&mut oversized, &services.game, "0").await;
    send(&mut oversized, json!({"padding":"x".repeat(1024)})).await;
    closed(&mut oversized).await;
    let mut rate = services.connect(&services.white).await;
    initial(&mut rate, &services.game, "0").await;
    for _ in 0..32 {
        if rate.send(Message::Text("{}".into())).await.is_err() {
            break;
        }
    }
    timeout(WAIT, async {
        loop {
            match rate.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break,
                Some(Ok(Message::Text(raw))) => {
                    let message: Value = serde_json::from_str(&raw).unwrap();
                    assert!(
                        message["type"] == "resync_required"
                            || (message["type"] == "error" && message["code"] == "invalid_input")
                    );
                }
                other => panic!("unexpected response to rate limit {other:?}"),
            }
        }
    })
    .await
    .expect("rate limit closes connection");
    assert_eq!(services.snapshot().await.seq, 0);
    services.stop().await;
}

#[tokio::test]
async fn in_flight_command_is_busy_until_its_response_is_enqueued() {
    let (captured, command_started) = oneshot::channel();
    let (release_command, release) = oneshot::channel();
    let mut services = Services::with_holds(
        Limits::default(),
        None,
        Some(CommandHold { captured, release }),
    )
    .await;
    let mut socket = services.connect(&services.black).await;
    initial(&mut socket, &services.game, "0").await;
    send(&mut socket, movement("held", "0", "7g7f")).await;
    timeout(WAIT, command_started).await.unwrap().unwrap();
    send(&mut socket, movement("busy", "0", "2g2f")).await;
    assert_eq!(
        receive(&mut socket).await,
        json!({"type":"error","request_id":"busy","code":"busy"})
    );
    assert_eq!(services.snapshot().await.seq, 0);
    release_command.send(()).unwrap();
    assert_eq!(receive(&mut socket).await["request_id"], "held");
    send(&mut socket, movement("released", "1", "3c3d")).await;
    let reply = receive(&mut socket).await;
    assert_eq!(reply["request_id"], "released");
    assert_eq!(reply["code"], "rejected");
    assert_eq!(services.snapshot().await.seq, 1);
    services.stop().await;
}
