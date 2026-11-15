crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Generated gRPC client over TCP, GameRpc, application and real PostgreSQL; test-only game seeding is fixture setup.",
        reason: "A legal TCP sequence asserts accepted capture/promotion/recapture/drop outcomes.",
        relationship: crate::trace_support::Relationship::Exercises,
        also: &[crate::trace_support::Evidence { feature: "game-play", contract: "domain-resign", scope: "Generated gRPC client over TCP, GameRpc, application and real PostgreSQL; test-only game seeding is fixture setup.", relationship: crate::trace_support::Relationship::Asserts, reason: "TCP resignation checks winner/reason and retained board/hands/turn with empty legal moves." }, crate::trace_support::Evidence { feature: "game-play", contract: "rpc-move", scope: "Generated gRPC client over TCP, GameRpc, application and real PostgreSQL; test-only game seeding is fixture setup.", relationship: crate::trace_support::Relationship::Asserts, reason: "Generated TCP client accepts capture/promotion/recapture/drop with sequence and result agreement." }, crate::trace_support::Evidence { feature: "game-play", contract: "rpc-resign", scope: "Generated gRPC client over TCP, GameRpc, application and real PostgreSQL; test-only game seeding is fixture setup.", relationship: crate::trace_support::Relationship::Asserts, reason: "TCP resignation returns expected acceptance and terminal result." }, crate::trace_support::Evidence { feature: "game-snapshots-access", contract: "grpc-shape", scope: "Generated gRPC client over TCP, GameRpc, application and real PostgreSQL; test-only game seeding is fixture setup.", relationship: crate::trace_support::Relationship::Asserts, reason: "TCP assertions check initial sequence/40 pieces/14 zero hands/Black turn/30 moves and selected capture/promotion/drop values; resignation retains position and returns result with empty legal moves." }],
    };
#[tokio::test]
async fn generated_client_plays_capture_promotion_recapture_drop_and_resignation() {
    let server = TestServer::start().await;
    let mut client = server.client().await;
    let initial = create(&mut client, "play").await;
    assert_eq!(initial.game_id, "play");
    assert_eq!(initial.seq, 0);
    assert_eq!(initial.turn, proto::Player::Black as i32);
    assert_eq!(initial.board.len(), 40);
    assert_eq!(initial.hands.len(), 14);
    assert!(initial.hands.iter().all(|hand| hand.count == 0));
    assert_eq!(piece(&initial, 5, 9).kind, proto::PieceKind::King as i32);
    assert!(!initial.in_check);
    assert!(initial.result.is_none());
    assert_eq!(initial.legal_moves.len(), 30);
    for (seq, player, usi) in [
        (0, proto::Player::Black, "7g7f"),
        (1, proto::Player::White, "3c3d"),
        (2, proto::Player::Black, "8h2b+"),
        (3, proto::Player::White, "3a2b"),
        (4, proto::Player::Black, "B*4e"),
    ] {
        let before = snapshot(&mut client, "play").await;
        assert!(before.legal_moves.iter().any(|legal| legal == usi));
        let receipt = client
            .make_move(command("play", &format!("m-{seq}"), seq, player, usi))
            .await
            .unwrap()
            .into_inner();
        let after = snapshot(&mut client, "play").await;
        assert_eq!(receipt.seq, seq + 1);
        assert_eq!(receipt.seq, after.seq);
        assert_eq!(receipt.in_check, after.in_check);
        assert_eq!(receipt.result, after.result);
        match seq {
            2 => {
                assert_eq!(piece(&after, 2, 2).kind, proto::PieceKind::Horse as i32);
                assert_eq!(piece(&after, 2, 2).owner, proto::Player::Black as i32);
                assert_eq!(
                    hand(&after, proto::Player::Black, proto::HandPiece::Bishop),
                    1
                );
            }
            3 => {
                assert_eq!(piece(&after, 2, 2).kind, proto::PieceKind::Silver as i32);
                assert_eq!(piece(&after, 2, 2).owner, proto::Player::White as i32);
                assert_eq!(
                    hand(&after, proto::Player::White, proto::HandPiece::Bishop),
                    1
                );
            }
            4 => {
                assert_eq!(piece(&after, 4, 5).kind, proto::PieceKind::Bishop as i32);
                assert_eq!(piece(&after, 4, 5).owner, proto::Player::Black as i32);
                assert_eq!(
                    hand(&after, proto::Player::Black, proto::HandPiece::Bishop),
                    0
                );
            }
            _ => {}
        }
    }
    let before = snapshot(&mut client, "play").await;
    let receipt = client
        .resign(resign("play", "resign", 5, proto::Player::White))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(receipt.seq, 6);
    assert_eq!(
        receipt.result,
        Some(proto::GameResult {
            winner: proto::Player::Black as i32,
            reason: proto::FinishReason::Resignation as i32
        })
    );
    let finished = snapshot(&mut client, "play").await;
    assert_eq!(finished.board, before.board);
    assert_eq!(finished.hands, before.hands);
    assert_eq!(finished.turn, before.turn);
    assert_eq!(finished.result, receipt.result);
    assert_eq!(finished.seq, receipt.seq);
    assert!(finished.legal_moves.is_empty());
    drop(client);
    server.shutdown().await;
}
}
