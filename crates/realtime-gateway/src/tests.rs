use super::*;
use services_support::{
    CommandHold, Services, SnapshotHold, Socket, WAIT, closed, initial, movement, send,
};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle, time::timeout};

async fn socket_pair() -> (WebSocket, Socket) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("ws://{}/", listener.local_addr().unwrap());
    let (socket_tx, socket_rx) = oneshot::channel();
    let socket_tx = Arc::new(Mutex::new(Some(socket_tx)));
    let router = Router::new().route(
        "/",
        get(move |ws: WebSocketUpgrade| async move {
            ws.on_upgrade(move |socket| async move {
                let _ = socket_tx.lock().unwrap().take().unwrap().send(socket);
            })
        }),
    );
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let (client, _) = tokio_tungstenite::connect_async(endpoint).await.unwrap();
    let socket = socket_rx.await.unwrap();
    server.abort();
    let _ = server.await;
    (socket, client)
}

async fn connection(services: &Services) -> (JoinHandle<()>, Socket) {
    let (socket, client) = socket_pair().await;
    let user = UserId::new(services.black.id.clone()).unwrap();
    let total = services.gateway.total.clone().try_acquire_owned().unwrap();
    let permit = services.gateway.permit(&user, total).unwrap();
    let session = Session {
        cookie: services.black.token.clone(),
        user,
    };
    let parent = tokio::spawn(run_connection(
        services.gateway.clone(),
        socket,
        GameId::new(services.game.clone()).unwrap(),
        session,
        permit,
    ));
    (parent, client)
}

async fn assert_released(services: &Services, client: &mut Socket) {
    assert_eq!(
        services.gateway.total.available_permits(),
        services.gateway.limits.connections
    );
    assert!(services.gateway.users.lock().unwrap().is_empty());
    closed(client).await;
    let game = GameId::new(services.game.clone()).unwrap();
    let mut broker = redis::Client::open(services.valkey_url.clone())
        .unwrap()
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    timeout(WAIT, async {
        loop {
            let (_, count): (String, i64) = redis::cmd("PUBSUB")
                .arg("NUMSUB")
                .arg(game_messaging::game_channel(&game))
                .query_async(&mut broker)
                .await
                .unwrap();
            if count == 0 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the cancelled connection releases its exact-game subscription");
}

#[traceability::targets(crate::run_connection)]
#[tokio::test]
async fn cancelling_parent_during_snapshot_releases_subscription_socket_and_capacity() {
    let (captured, ready) = oneshot::channel();
    let (release_snapshot, release) = oneshot::channel();
    let services =
        Services::with_snapshot_hold(Limits::default(), Some(SnapshotHold { captured, release }))
            .await;
    let (parent, mut client) = connection(&services).await;
    timeout(WAIT, ready).await.unwrap().unwrap();
    parent.abort();
    assert!(parent.await.unwrap_err().is_cancelled());
    assert_released(&services, &mut client).await;
    let _ = release_snapshot.send(());
    services.stop().await;
}

#[traceability::targets(crate::run_connection)]
#[tokio::test]
async fn cancelling_parent_during_command_releases_all_connection_owned_futures() {
    let (captured, ready) = oneshot::channel();
    let (release_command, release) = oneshot::channel();
    let services = Services::with_holds(
        Limits::default(),
        None,
        Some(CommandHold { captured, release }),
    )
    .await;
    let (parent, mut client) = connection(&services).await;
    initial(&mut client, &services.game, "0").await;
    send(&mut client, movement("cancelled-owner", "0", "7g7f")).await;
    timeout(WAIT, ready).await.unwrap().unwrap();
    parent.abort();
    assert!(parent.await.unwrap_err().is_cancelled());
    assert_released(&services, &mut client).await;
    let _ = release_command.send(());
    services.stop().await;
}

#[traceability::targets(crate::reader)]
#[tokio::test]
async fn reader_output_overflow_is_terminal() {
    let (socket, mut client) = socket_pair().await;
    let (_sink, stream) = socket.split();
    let (outgoing, mut output) = mpsc::channel(1);
    let (commands, _pending) = mpsc::channel(1);
    let limits = Limits::default();
    let reader = reader(
        stream,
        commands,
        outgoing,
        Arc::new(Semaphore::new(1)),
        &limits,
    );
    tokio::pin!(reader);
    send(&mut client, json!({})).await;
    send(&mut client, json!({})).await;
    assert!(matches!(
        timeout(WAIT, &mut reader).await.unwrap(),
        ReaderExit::Overflow
    ));
    assert!(
        matches!(output.recv().await, Some(Outbound::Data(value)) if value["code"] == "invalid_input")
    );
}

#[traceability::targets(crate::command_worker)]
#[tokio::test]
async fn dropping_worker_during_rpc_releases_its_command_permit_before_writer_drain() {
    let (captured, ready) = oneshot::channel();
    let (release_command, release) = oneshot::channel();
    let services = Services::with_holds(
        Limits::default(),
        None,
        Some(CommandHold { captured, release }),
    )
    .await;
    let game = GameId::new(services.game.clone()).unwrap();
    let session = Session {
        cookie: services.black.token.clone(),
        user: UserId::new(services.black.id.clone()).unwrap(),
    };
    let one_command = Arc::new(Semaphore::new(1));
    let (commands, pending) = mpsc::channel(1);
    let (outgoing, mut output) = mpsc::channel(1);
    commands
        .try_send((
            parse_command(&movement("dropped-worker", "0", "7g7f").to_string()).unwrap(),
            one_command.clone().try_acquire_owned().unwrap(),
        ))
        .unwrap();
    {
        let worker = command_worker(&services.gateway, &game, &session, pending, outgoing);
        tokio::pin!(worker);
        tokio::select! {
            _ = &mut worker => panic!("worker must remain in the held RPC"),
            result = ready => result.unwrap(),
        }
        assert_eq!(one_command.available_permits(), 0);
    }
    assert_eq!(one_command.available_permits(), 1);
    assert!(
        matches!(
            output.try_recv(),
            Err(mpsc::error::TryRecvError::Disconnected)
        ),
        "a dropped worker cannot enqueue a response during drain"
    );
    let _ = release_command.send(());
    services.stop().await;
}

#[traceability::targets(crate::run_connection)]
#[tokio::test]
async fn writer_completion_ends_parent_without_waiting_for_reader_or_polling_writer_again() {
    let services = Services::start().await;
    let (parent, mut client) = connection(&services).await;
    initial(&mut client, &services.game, "0").await;
    let response = reqwest::Client::new()
        .post(format!("{}/auth/logout", services.user_url))
        .header("origin", &services.origin)
        .header("cookie", services.black.cookie())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    send(&mut client, json!({})).await;
    timeout(WAIT, parent)
        .await
        .unwrap()
        .expect("completed writer is never polled again");
    assert_released(&services, &mut client).await;
    services.stop().await;
}

#[traceability::targets(crate::command_worker)]
#[tokio::test]
async fn full_command_output_releases_permit_and_reports_overflow_after_commit() {
    let mut services = Services::start().await;
    let game = GameId::new(services.game.clone()).unwrap();
    let session = Session {
        cookie: services.black.token.clone(),
        user: UserId::new(services.black.id.clone()).unwrap(),
    };
    let one_command = Arc::new(Semaphore::new(1));
    let (commands, pending) = mpsc::channel(1);
    let (outgoing, mut output) = mpsc::channel(1);
    assert!(
        outgoing
            .try_send(Outbound::Data(json!({"queued":true})))
            .is_ok()
    );
    commands
        .try_send((
            parse_command(&movement("full-output", "0", "7g7f").to_string()).unwrap(),
            one_command.clone().try_acquire_owned().unwrap(),
        ))
        .unwrap();
    let result = timeout(
        WAIT,
        command_worker(&services.gateway, &game, &session, pending, outgoing),
    )
    .await
    .unwrap();
    assert!(matches!(result, WorkerExit::Overflow));
    assert_eq!(one_command.available_permits(), 1);
    assert!(
        matches!(output.recv().await, Some(Outbound::Data(value)) if value == json!({"queued":true}))
    );
    assert!(output.recv().await.is_none());
    assert_eq!(services.snapshot().await.seq, 1);
    services.stop().await;
}
