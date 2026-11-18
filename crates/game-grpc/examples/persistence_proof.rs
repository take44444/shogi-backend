mod support;
use game_grpc::proto::{self, game_service_client::GameServiceClient};
use prost::Message;
use std::{fs, path::Path};
use tonic::transport::Channel;

type Client = GameServiceClient<Channel>;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn movement(identity: &proto::Snapshot, game: &str, seq: u64, usi: &str) -> proto::MakeMoveRequest {
    proto::MakeMoveRequest {
        game_id: game.into(),
        request_id: format!("move-{seq}"),
        expected_seq: Some(seq),
        user_id: if seq.is_multiple_of(2) {
            identity.black_user_id.clone()
        } else {
            identity.white_user_id.clone()
        },
        usi: usi.into(),
    }
}
fn resignation(identity: &proto::Snapshot, game: &str) -> proto::ResignRequest {
    proto::ResignRequest {
        game_id: game.into(),
        request_id: "resign".into(),
        expected_seq: Some(5),
        user_id: identity.white_user_id.clone(),
    }
}
async fn snapshot(client: &mut Client, game: &str) -> Result<proto::Snapshot> {
    Ok(client
        .get_snapshot(proto::GetSnapshotRequest {
            game_id: game.into(),
        })
        .await?
        .into_inner())
}
fn save(path: &Path, value: &impl Message) -> Result<()> {
    fs::write(path, value.encode_to_vec())?;
    Ok(())
}
fn load<T: Message + Default>(path: &Path) -> Result<T> {
    Ok(T::decode(fs::read(path)?.as_slice())?)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err(
            "usage: persistence_proof seed|resume|finished ENDPOINT GAME_ID PROOF_DIRECTORY".into(),
        );
    }
    let mode = args[0].as_str();
    let game = &args[2];
    let dir = Path::new(&args[3]);
    let mut client = Client::connect(args[1].clone()).await?;
    match mode {
        "seed" => {
            fs::create_dir_all(dir)?;
            let initial = support::pair(
                &mut client,
                [&format!("{game}-one"), &format!("{game}-two")],
            )
            .await?;
            save(&dir.join("identity.bin"), &initial)?;
            let game = &initial.game_id;
            let identity = initial.clone();
            assert_eq!(initial.seq, 0);
            for (seq, usi) in ["7g7f", "3c3d", "8h2b+"].into_iter().enumerate() {
                let receipt = client
                    .make_move(movement(&identity, game, seq as u64, usi))
                    .await?
                    .into_inner();
                assert_eq!(receipt.seq, seq as u64 + 1);
                if seq == 0 {
                    save(&dir.join("first.bin"), &receipt)?;
                }
            }
            let ongoing = snapshot(&mut client, game).await?;
            assert_eq!(ongoing.seq, 3);
            assert!(ongoing.result.is_none());
            save(&dir.join("ongoing.bin"), &ongoing)?;
            println!("seed verified at sequence 3; restart or recreate the server before resume");
        }
        "resume" => {
            let identity: proto::Snapshot = load(&dir.join("identity.bin"))?;
            let game = &identity.game_id;
            let expected: proto::Snapshot = load(&dir.join("ongoing.bin"))?;
            assert_eq!(snapshot(&mut client, game).await?, expected);
            let first: proto::Acceptance = load(&dir.join("first.bin"))?;
            assert_eq!(
                client
                    .make_move(movement(&identity, game, 0, "7g7f"))
                    .await?
                    .into_inner(),
                first
            );
            for (seq, usi) in [(3, "3a2b"), (4, "B*4e")] {
                let receipt = client
                    .make_move(movement(&identity, game, seq, usi))
                    .await?
                    .into_inner();
                assert_eq!(receipt.seq, seq + 1);
            }
            let before = snapshot(&mut client, game).await?;
            let finish = client
                .resign(resignation(&identity, game))
                .await?
                .into_inner();
            assert_eq!(finish.seq, 6);
            assert_eq!(
                finish.result,
                Some(proto::GameResult {
                    winner: proto::Player::Black as i32,
                    reason: proto::FinishReason::Resignation as i32
                })
            );
            let finished = snapshot(&mut client, game).await?;
            assert_eq!(finished.board, before.board);
            assert_eq!(finished.hands, before.hands);
            assert_eq!(finished.turn, before.turn);
            assert!(finished.legal_moves.is_empty());
            assert_eq!(finished.result, finish.result);
            assert_eq!(finished.seq, 6);
            save(&dir.join("finished.bin"), &finished)?;
            save(&dir.join("resign.bin"), &finish)?;
            println!(
                "resume verified exact snapshot and replay; continued and finished at sequence 6; restart again before finished"
            );
        }
        "finished" => {
            let identity: proto::Snapshot = load(&dir.join("identity.bin"))?;
            let game = &identity.game_id;
            let expected: proto::Snapshot = load(&dir.join("finished.bin"))?;
            assert_eq!(snapshot(&mut client, game).await?, expected);
            let first: proto::Acceptance = load(&dir.join("first.bin"))?;
            let finish: proto::Acceptance = load(&dir.join("resign.bin"))?;
            assert_eq!(
                client
                    .make_move(movement(&identity, game, 0, "7g7f"))
                    .await?
                    .into_inner(),
                first
            );
            assert_eq!(
                client
                    .resign(resignation(&identity, game))
                    .await?
                    .into_inner(),
                finish
            );
            assert_eq!(snapshot(&mut client, game).await?, expected);
            println!(
                "finished restart verified exact snapshot and both original receipts at sequence 6"
            );
        }
        _ => return Err("mode must be seed, resume, or finished".into()),
    }
    Ok(())
}
