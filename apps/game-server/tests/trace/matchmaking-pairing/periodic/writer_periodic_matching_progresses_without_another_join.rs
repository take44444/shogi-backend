crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Actual game-server process, real PostgreSQL and generated gRPC client; DB timestamps force eligibility.",
        reason: "Launches the actual game-server binary and observes waiting tickets become Playing with a sequence-zero snapshot containing both users, without another join.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn writer_periodic_matching_progresses_without_another_join() {
    let database = TestDatabase::new().await;
    SeaOrmStore::connect(database.url())
        .await
        .unwrap()
        .close()
        .await
        .unwrap();
    let raw = Database::connect(database.url()).await.unwrap();
    raw.execute_unprepared("INSERT INTO rating_states(user_id,pool,rating,completed) VALUES ('one','ten_minute_30',1500,1),('two','ten_minute_30',1650,1)").await.unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let _process = Process(
        Command::new(env!("CARGO_BIN_EXE_game-server"))
            .arg(address.to_string())
            .env("DATABASE_URL", database.url())
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut client = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match GameServiceClient::connect(format!("http://{address}")).await {
                Ok(client) => break client,
                Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
            }
        }
    })
    .await
    .unwrap();
    let id = TicketId::new().to_string();
    for (user, ticket) in [("one", id.clone()), ("two", TicketId::new().to_string())] {
        assert!(
            client
                .join_matchmaking(proto::JoinMatchmakingRequest {
                    user_id: user.into(),
                    ticket_id: ticket,
                    pool: proto::RatingPool::TenMinute30 as i32
                })
                .await
                .unwrap()
                .into_inner()
                .created
        );
    }
    let lookup = proto::MatchmakingRequest {
        user_id: "one".into(),
        ticket_id: id,
    };
    assert_eq!(
        client
            .get_matchmaking_ticket(lookup.clone())
            .await
            .unwrap()
            .into_inner()
            .state,
        "waiting"
    );
    raw.execute_unprepared(
        "UPDATE matchmaking_tickets SET enqueued_at=enqueued_at-30000 WHERE state='waiting'",
    )
    .await
    .unwrap();
    let ticket = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let ticket = client
                .get_matchmaking_ticket(lookup.clone())
                .await
                .unwrap()
                .into_inner();
            if ticket.state == "playing" {
                break ticket;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let game = client
        .get_snapshot(proto::GetSnapshotRequest {
            game_id: ticket.game_id.unwrap(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(game.seq, 0);
    assert!([game.black_user_id.as_str(), game.white_user_id.as_str()].contains(&"one"));
    assert!([game.black_user_id.as_str(), game.white_user_id.as_str()].contains(&"two"));
}
}
