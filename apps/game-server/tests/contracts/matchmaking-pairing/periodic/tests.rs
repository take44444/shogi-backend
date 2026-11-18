use game_application::matchmaking::TicketId;
use game_grpc::proto::{self, game_service_client::GameServiceClient};
use game_persistence::SeaOrmStore;
use pg_test_support::TestDatabase;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use std::{
    process::{Child, Command, Stdio},
    time::Duration,
};

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/matchmaking-tick", relationship: crate::trace_support::Relationship::Asserts, scope: "実際のgame-serverプロセス、PostgreSQL、生成されたgRPCクライアントを使う。DBの時刻を操作してマッチング条件を満たすようにする。", reason: "実際のgame-server実行ファイルを起動する。追加の参加要求がなくても、待機中のチケットがMatchedとなり、両者の準備完了後にPlayingとなり、両ユーザーを含む連番0のスナップショットを取得できることを確認する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-persistence"], fake: &[] }] };
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
    let other = TicketId::new().to_string();
    for (user, ticket) in [("one", id.clone()), ("two", other.clone())] {
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
    let command = proto::MatchmakingRequest {
        user_id: "one".into(),
        ticket_id: id.clone(),
    };
    let resumed = client
        .get_matchmaking(proto::MatchmakingOwner {
            user_id: "one".into(),
        })
        .await
        .unwrap()
        .into_inner()
        .ticket
        .unwrap();
    assert_eq!(resumed.ticket_id, id);
    assert_eq!(resumed.state, "waiting");
    assert_eq!(resumed.heartbeat_sequence, Some(0));
    raw.execute_unprepared(
        "UPDATE matchmaking_tickets SET enqueued_at=enqueued_at-30000 WHERE state='waiting'",
    )
    .await
    .unwrap();
    let ticket = tokio::time::timeout(Duration::from_secs(5), async {
        let mut sequence = 1;
        loop {
            let mut first = None;
            for (user, id) in [("one", &id), ("two", &other)] {
                let ticket = client
                    .heartbeat_matchmaking(proto::HeartbeatMatchmakingRequest {
                        user_id: user.into(),
                        ticket_id: id.clone(),
                        sequence,
                    })
                    .await
                    .unwrap()
                    .into_inner();
                assert!(matches!(ticket.state.as_str(), "waiting" | "matched"));
                if ticket.state == "waiting" {
                    assert_eq!(ticket.heartbeat_sequence, Some(sequence));
                }
                if first.is_none() {
                    first = Some(ticket);
                }
            }
            sequence += 1;
            let ticket = first.unwrap();
            if ticket.state == "matched" {
                break ticket;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(ticket.ready, Some(false));
    assert!(ticket.ready_deadline.is_some());
    let game_id = ticket.game_id.unwrap();
    assert_eq!(
        client
            .get_snapshot(proto::GetSnapshotRequest {
                game_id: game_id.clone(),
            })
            .await
            .unwrap_err()
            .code(),
        tonic::Code::NotFound
    );
    let first = client.ready_matchmaking(command).await.unwrap().into_inner();
    assert_eq!(first.state, "matched");
    assert_eq!(first.ready, Some(true));
    let second = client
        .ready_matchmaking(proto::MatchmakingRequest {
            user_id: "two".into(),
            ticket_id: other,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(second.state, "playing");
    let game = client
        .get_snapshot(proto::GetSnapshotRequest { game_id })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(game.seq, 0);
    assert!([game.black_user_id.as_str(), game.white_user_id.as_str()].contains(&"one"));
    assert!([game.black_user_id.as_str(), game.white_user_id.as_str()].contains(&"two"));
    let pending = [TicketId::new().to_string(), TicketId::new().to_string()];
    for (user, id) in ["three", "four"].iter().zip(&pending) {
        client
            .join_matchmaking(proto::JoinMatchmakingRequest {
                user_id: (*user).into(),
                ticket_id: id.clone(),
                pool: proto::RatingPool::Byoyomi30 as i32,
            })
            .await
            .unwrap();
    }
    let (matched, next_sequence) = tokio::time::timeout(Duration::from_secs(5), async {
        let mut sequence = 1;
        loop {
            let mut first = None;
            for (user, id) in [("three", &pending[0]), ("four", &pending[1])] {
                let ticket = client
                    .heartbeat_matchmaking(proto::HeartbeatMatchmakingRequest {
                        user_id: user.into(),
                        ticket_id: id.clone(),
                        sequence,
                    })
                    .await
                    .unwrap()
                    .into_inner();
                assert!(matches!(ticket.state.as_str(), "waiting" | "matched"));
                if ticket.state == "waiting" {
                    assert_eq!(ticket.heartbeat_sequence, Some(sequence));
                }
                if first.is_none() {
                    first = Some(ticket);
                }
            }
            sequence += 1;
            let ticket = first.unwrap();
            if ticket.state == "matched" {
                break (ticket, sequence);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let prospective = matched.game_id.unwrap();
    raw.execute_unprepared("UPDATE matchmaking_tickets SET ready_deadline=0 WHERE state='matched'")
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let row = raw.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
                "SELECT count(*) AS n FROM matchmaking_tickets WHERE prospective_game_id=$1 AND state='voided'",
                [prospective.clone().into()],
            )).await.unwrap().unwrap();
            if row.try_get::<i64>("", "n").unwrap() == 2 { break; }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }).await.expect("periodic matching must expire the pair without an RPC request");
    let row = raw.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "SELECT (SELECT count(*) FROM games WHERE id=$1)+(SELECT count(*) FROM rated_games WHERE game_id=$1)+(SELECT count(*) FROM rating_slots WHERE game_id=$1) AS n",
        [prospective.clone().into()],
    )).await.unwrap().unwrap();
    assert_eq!(row.try_get::<i64>("", "n").unwrap(), 0);
    for (user, id) in ["three", "four"].iter().zip(pending) {
        let voided = client
            .heartbeat_matchmaking(proto::HeartbeatMatchmakingRequest {
                user_id: (*user).into(),
                ticket_id: id,
                sequence: next_sequence,
            })
            .await
            .unwrap()
            .into_inner();
        assert_eq!(voided.state, "voided");
        assert_eq!(voided.game_id.as_deref(), Some(prospective.as_str()));
        assert!(voided.ready.is_none());
        assert!(voided.ready_deadline.is_none());
        assert!(
            client
                .get_matchmaking(proto::MatchmakingOwner {
                    user_id: (*user).into()
                })
                .await
                .unwrap()
                .into_inner()
                .ticket
                .is_none()
        );
    }
    raw.close().await.unwrap();
}
}
