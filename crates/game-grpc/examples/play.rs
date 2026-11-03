//! Drives a complete game through a running gRPC server using a new game and caller-supplied participant IDs.

use game_grpc::proto::{self, game_service_client::GameServiceClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let endpoint = args
        .next()
        .unwrap_or_else(|| "http://127.0.0.1:50051".to_owned());
    let game_id = args.next().unwrap_or_else(|| {
        format!(
            "demo-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    });
    let mut client = GameServiceClient::connect(endpoint).await?;
    let black = format!("{game_id}-black");
    let white = format!("{game_id}-white");
    let mut snapshot = client
        .create_game(proto::CreateGameRequest {
            game_id: game_id.clone(),
            black_user_id: black.clone(),
            white_user_id: white.clone(),
        })
        .await?
        .into_inner();
    println!("created {} at sequence {}", snapshot.game_id, snapshot.seq);
    assert_eq!(snapshot.seq, 0);
    assert_eq!(snapshot.board.len(), 40);
    assert_eq!(snapshot.hands.len(), 14);
    let mut first = None;
    for (index, usi) in ["7g7f", "3c3d", "8h2b+", "3a2b", "B*4e"]
        .into_iter()
        .enumerate()
    {
        assert!(
            snapshot.legal_moves.iter().any(|legal| legal == usi),
            "{usi} must be legal"
        );
        let command = proto::MakeMoveRequest {
            game_id: game_id.clone(),
            request_id: format!("move-{index}"),
            expected_seq: Some(snapshot.seq),
            user_id: if snapshot.turn == proto::Player::Black as i32 {
                black.clone()
            } else {
                white.clone()
            },
            usi: usi.to_owned(),
        };
        let receipt = client.make_move(command.clone()).await?.into_inner();
        assert_eq!(receipt.seq, snapshot.seq + 1);
        assert!(receipt.result.is_none());
        if index == 0 {
            first = Some((command, receipt));
        }
        snapshot = client
            .get_snapshot(proto::GetSnapshotRequest {
                game_id: game_id.clone(),
            })
            .await?
            .into_inner();
        assert_eq!(snapshot.seq, receipt.seq);
        assert_eq!(snapshot.in_check, receipt.in_check);
        match index {
            2 => {
                assert_piece(
                    &snapshot,
                    2,
                    2,
                    proto::Player::Black,
                    proto::PieceKind::Horse,
                );
                assert_hand(&snapshot, proto::Player::Black, 1);
                println!("capture and promotion verified at 2b");
            }
            3 => {
                assert_piece(
                    &snapshot,
                    2,
                    2,
                    proto::Player::White,
                    proto::PieceKind::Silver,
                );
                assert_hand(&snapshot, proto::Player::White, 1);
                println!("recapture demotes the horse into a bishop in hand");
            }
            4 => {
                assert_piece(
                    &snapshot,
                    4,
                    5,
                    proto::Player::Black,
                    proto::PieceKind::Bishop,
                );
                assert_hand(&snapshot, proto::Player::Black, 0);
                println!("bishop drop verified at 4e");
            }
            _ => {}
        }
        println!("accepted {usi}, sequence {}", receipt.seq);
    }
    let resign = proto::ResignRequest {
        game_id: game_id.clone(),
        request_id: "resign".into(),
        expected_seq: Some(snapshot.seq),
        user_id: white,
    };
    let receipt = client.resign(resign.clone()).await?.into_inner();
    assert_eq!(receipt.seq, 6);
    assert_eq!(
        receipt.result,
        Some(proto::GameResult {
            winner: proto::Player::Black as i32,
            reason: proto::FinishReason::Resignation as i32
        })
    );
    assert_eq!(client.resign(resign).await?.into_inner(), receipt);
    let (command, original_receipt) = first.unwrap();
    assert_eq!(
        client.make_move(command).await?.into_inner(),
        original_receipt
    );
    let finished = client
        .get_snapshot(proto::GetSnapshotRequest { game_id })
        .await?
        .into_inner();
    assert_eq!(finished.seq, 6);
    assert_eq!(finished.result, receipt.result);
    assert!(finished.legal_moves.is_empty());
    assert_eq!(finished.board, snapshot.board);
    assert_eq!(finished.hands, snapshot.hands);
    println!("resignation and exact retries verified; black wins at sequence 6");
    Ok(())
}

fn assert_piece(
    snapshot: &proto::Snapshot,
    file: u32,
    rank: u32,
    owner: proto::Player,
    kind: proto::PieceKind,
) {
    assert!(snapshot.board.iter().any(|piece| piece.file == file
        && piece.rank == rank
        && piece.owner == owner as i32
        && piece.kind == kind as i32));
}

fn assert_hand(snapshot: &proto::Snapshot, player: proto::Player, count: u32) {
    let hand = snapshot
        .hands
        .iter()
        .find(|hand| hand.player == player as i32 && hand.piece == proto::HandPiece::Bishop as i32)
        .unwrap();
    assert_eq!(hand.count, count);
}
