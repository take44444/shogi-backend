use futures_util::StreamExt;
use game_application::GameId;
use game_grpc::proto::{self, game_service_client::GameServiceClient};
use redis::aio::PubSub;
use serde_json::{Value, json};
use std::{process::Command, time::Duration};
use tonic::transport::Channel;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type Client = GameServiceClient<Channel>;

fn compose(project: &str, args: &[&str]) -> Result<()> {
    let status = Command::new("docker")
        .args(["compose", "-p", project])
        .args(args)
        .status()?;
    if !status.success() {
        return Err(format!("Compose {args:?} failed: {status}").into());
    }
    Ok(())
}
fn publisher_restarts(project: &str) -> Result<u64> {
    let id = Command::new("docker")
        .args([
            "compose",
            "-p",
            project,
            "ps",
            "--all",
            "--quiet",
            "outbox-publisher",
        ])
        .output()?;
    if !id.status.success() {
        return Err("could not find publisher container".into());
    }
    let id = String::from_utf8(id.stdout)?;
    let state = Command::new("docker")
        .args(["inspect", "--format", "{{.RestartCount}}", id.trim()])
        .output()?;
    if !state.status.success() {
        return Err("could not inspect publisher restarts".into());
    }
    Ok(String::from_utf8(state.stdout)?.trim().parse()?)
}
async fn connect(endpoint: &str) -> Result<Client> {
    for _ in 0..100 {
        if let Ok(mut client) = Client::connect(endpoint.to_owned()).await {
            let response = client
                .get_snapshot(proto::GetSnapshotRequest {
                    game_id: "outbox-readiness-probe".into(),
                })
                .await;
            if response.is_ok()
                || response.is_err_and(|error| error.code() == tonic::Code::NotFound)
            {
                return Ok(client);
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err("gRPC server did not become ready".into())
}
async fn subscribe(url: &str, game: &str) -> Result<PubSub> {
    let client = redis::Client::open(url)?;
    for _ in 0..100 {
        if let Ok(mut subscriber) = client.get_async_pubsub().await {
            subscriber
                .subscribe(game_messaging::game_channel(&GameId::new(game)?))
                .await?;
            return Ok(subscriber);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err("Valkey did not become ready".into())
}
async fn create(client: &mut Client, game: &str) -> Result<()> {
    let response = client
        .create_game(proto::CreateGameRequest {
            game_id: game.into(),
            black_user_id: format!("{game}-black"),
            white_user_id: format!("{game}-white"),
        })
        .await?
        .into_inner();
    assert_eq!(response.seq, 0);
    Ok(())
}
fn movement(game: &str, seq: u64, usi: &str) -> proto::MakeMoveRequest {
    proto::MakeMoveRequest {
        game_id: game.into(),
        request_id: format!("move-{seq}"),
        expected_seq: Some(seq),
        user_id: format!(
            "{game}-{}",
            if seq.is_multiple_of(2) {
                "black"
            } else {
                "white"
            }
        ),
        usi: usi.into(),
    }
}
async fn receive(sub: &mut PubSub, game: &str, seq: u64, kind: &str) -> Result<Value> {
    let message = tokio::time::timeout(Duration::from_secs(20), sub.on_message().next())
        .await?
        .ok_or("subscription ended")?;
    assert_eq!(
        message.get_channel_name(),
        game_messaging::game_channel(&GameId::new(game)?)
    );
    let value: Value = serde_json::from_slice(message.get_payload_bytes())?;
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["game_id"], game);
    assert_eq!(value["seq"], seq);
    assert_eq!(value["type"], kind);
    let mut encoded = String::new();
    use std::fmt::Write;
    for byte in game.bytes() {
        write!(&mut encoded, "{byte:02x}")?;
    }
    assert_eq!(value["event_id"], format!("{encoded}:{seq}:{kind}"));
    println!("received {}", value);
    Ok(value)
}
async fn quiet(sub: &mut PubSub) -> Result<()> {
    assert!(
        tokio::time::timeout(Duration::from_millis(1200), sub.on_message().next())
            .await
            .is_err(),
        "unexpected duplicate or premature event"
    );
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: outbox_proof GRPC_ENDPOINT VALKEY_URL COMPOSE_PROJECT".into());
    }
    let (endpoint, valkey, project) = (&args[0], &args[1], &args[2]);
    let mut client = connect(endpoint).await?;
    let game = "proof.*.>棋\0";
    let mut sub = subscribe(valkey, game).await?;
    create(&mut client, game).await?;
    let first = client
        .make_move(movement(game, 0, "7g7f"))
        .await?
        .into_inner();
    assert_eq!(first.seq, 1);
    let normal = receive(&mut sub, game, 1, "move_committed").await?;
    assert_eq!(normal["movement"], "7g7f");
    assert_eq!(normal["in_check"], false);
    assert_eq!(normal["result"], Value::Null);
    assert_eq!(normal["request_id"], "move-0");
    assert_eq!(normal["user_id"], format!("{game}-black"));
    assert_eq!(
        client
            .make_move(movement(game, 0, "7g7f"))
            .await?
            .into_inner(),
        first
    );
    client
        .resign(proto::ResignRequest {
            game_id: game.into(),
            request_id: "end".into(),
            expected_seq: Some(1),
            user_id: format!("{game}-white"),
        })
        .await?;
    let end = receive(&mut sub, game, 2, "game_finished").await?;
    assert_eq!(
        end["result"],
        json!({"winner":"black","reason":"resignation"})
    );
    assert!(end.get("movement").is_none());
    quiet(&mut sub).await?;
    println!("normal move, exact retry, resignation, and safe channel verified");

    let game = "backlog";
    let mut sub = subscribe(valkey, game).await?;
    create(&mut client, game).await?;
    compose(project, &["stop", "outbox-publisher"])?;
    for (seq, usi) in [(0, "7g7f"), (1, "3c3d")] {
        assert_eq!(
            client
                .make_move(movement(game, seq, usi))
                .await?
                .into_inner()
                .seq,
            seq + 1
        );
    }
    quiet(&mut sub).await?;
    compose(
        project,
        &["up", "-d", "--no-deps", "--force-recreate", "game-server"],
    )?;
    client = connect(endpoint).await?;
    assert_eq!(
        client
            .get_snapshot(proto::GetSnapshotRequest {
                game_id: game.into()
            })
            .await?
            .into_inner()
            .seq,
        2
    );
    assert_eq!(
        client
            .make_move(movement(game, 0, "7g7f"))
            .await?
            .into_inner()
            .seq,
        1
    );
    compose(project, &["start", "outbox-publisher"])?;
    receive(&mut sub, game, 1, "move_committed").await?;
    receive(&mut sub, game, 2, "move_committed").await?;
    quiet(&mut sub).await?;
    println!(
        "publisher downtime backlog survived server recreation, replayed in order, and exact retry added no event"
    );

    let restarts = publisher_restarts(project)?;
    compose(project, &["stop", "valkey"])?;
    assert_eq!(
        client
            .make_move(movement(game, 2, "2g2f"))
            .await?
            .into_inner()
            .seq,
        3
    );
    assert_eq!(
        client
            .get_snapshot(proto::GetSnapshotRequest {
                game_id: game.into()
            })
            .await?
            .into_inner()
            .seq,
        3
    );
    tokio::time::timeout(Duration::from_secs(20), async {
        while publisher_restarts(project)? <= restarts {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    })
    .await??;
    println!("failed publication triggered automatic publisher restart during Valkey outage");
    compose(project, &["stop", "outbox-publisher"])?;
    compose(project, &["start", "valkey"])?;
    let mut fresh_sub = subscribe(valkey, game).await?;
    compose(project, &["start", "outbox-publisher"])?;
    receive(&mut fresh_sub, game, 3, "move_committed").await?;
    quiet(&mut fresh_sub).await?;
    println!("RPC succeeded during Valkey outage; pending event replayed after recovery");
    compose(project, &["restart", "outbox-publisher"])?;
    quiet(&mut fresh_sub).await?;
    assert_eq!(
        client
            .make_move(movement(game, 2, "2g2f"))
            .await?
            .into_inner()
            .seq,
        3
    );
    quiet(&mut fresh_sub).await?;
    assert_eq!(
        client
            .make_move(movement(game, 3, "8c8d"))
            .await?
            .into_inner()
            .seq,
        4
    );
    receive(&mut fresh_sub, game, 4, "move_committed").await?;
    println!(
        "publisher restart and request replay did not republish marked rows; new events still flow"
    );
    let game = "unwatched";
    create(&mut client, game).await?;
    let mut check = redis::Client::open(valkey.as_str())?
        .get_multiplexed_async_connection()
        .await?;
    let (_, subscribers): (String, u64) = redis::cmd("PUBSUB")
        .arg("NUMSUB")
        .arg(game_messaging::game_channel(&GameId::new(game)?))
        .query_async(&mut check)
        .await?;
    assert_eq!(subscribers, 0);
    client.make_move(movement(game, 0, "7g7f")).await?;
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let output = Command::new("docker")
                .args([
                    "compose", "-p", project, "exec", "-T", "postgres", "psql",
                    "-U", "shogi", "-d", "shogi", "-Atc",
                    "SELECT COUNT(*) FROM outbox_events WHERE game_id = 'unwatched' AND published = true",
                ])
                .output()?;
            if !output.status.success() {
                return Err("could not inspect outbox marking".into());
            }
            if String::from_utf8(output.stdout)?.trim() == "1" {
                return Ok::<(), Box<dyn std::error::Error>>(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await??;
    let mut unwatched = subscribe(valkey, game).await?;
    compose(project, &["restart", "outbox-publisher"])?;
    quiet(&mut unwatched).await?;
    assert_eq!(
        client
            .get_snapshot(proto::GetSnapshotRequest {
                game_id: game.into()
            })
            .await?
            .into_inner()
            .seq,
        1
    );
    println!(
        "zero subscribers still marked acceptance; later subscription did not replay; snapshot recovered state"
    );
    Ok(())
}
