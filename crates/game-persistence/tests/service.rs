use game_application::{
    ApplicationError as Error, GameId, GameService, MakeMove, Participants, RequestId, Resign,
    UserId,
};
use pg_test_support::TestDatabase;
use shogi_domain::{CommandError, FinishReason, Player};
use std::sync::Arc;
use tokio::{sync::Barrier, task::JoinSet};

fn id(value: &str) -> GameId {
    GameId::new(value).unwrap()
}
fn movement(request: &str, seq: u64, player: Player, usi: &str) -> MakeMove {
    MakeMove {
        request_id: RequestId::new(request).unwrap(),
        expected_seq: seq,
        user_id: user(player),
        movement: usi.parse().unwrap(),
    }
}
fn resignation(request: &str, seq: u64, player: Player) -> Resign {
    Resign {
        request_id: RequestId::new(request).unwrap(),
        expected_seq: seq,
        user_id: user(player),
    }
}

#[test]
fn ids_validate_bytes_and_unicode_whitespace() {
    for invalid in [
        "".to_owned(),
        " ".into(),
        "x\ty".into(),
        "x\ny".into(),
        "x\u{2003}y".into(),
        "x".repeat(129),
        "é".repeat(65),
    ] {
        assert!(GameId::new(invalid.clone()).is_err());
        assert!(RequestId::new(invalid).is_err());
    }
    for valid in ["game-1".to_owned(), "x".repeat(128), "é".repeat(64)] {
        assert_eq!(GameId::new(valid.clone()).unwrap().as_str(), valid);
        assert_eq!(RequestId::new(valid.clone()).unwrap().as_str(), valid);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_creation_has_exactly_one_winner_and_shared_state() {
    let (_database, service) = setup().await;
    let barrier = Arc::new(Barrier::new(24));
    let mut tasks = JoinSet::new();
    for _ in 0..24 {
        let service = service.clone();
        let barrier = barrier.clone();
        tasks.spawn(async move {
            barrier.wait().await;
            service.create(id("same"), participants()).await
        });
    }
    let mut created = 0;
    while let Some(result) = tasks.join_next().await {
        match result.unwrap() {
            Ok(snapshot) => {
                created += 1;
                assert_eq!(snapshot.seq, 0);
            }
            Err(error) => assert_eq!(error, Error::AlreadyExists),
        }
    }
    assert_eq!(created, 1);
    service
        .clone()
        .make_move(&id("same"), movement("m", 0, Player::Black, "7g7f"))
        .await
        .unwrap();
    assert_eq!(service.snapshot(&id("same")).await.unwrap().seq, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn competing_commands_at_one_sequence_accept_exactly_one() {
    let (_database, service) = setup().await;
    let game = id("race");
    service.create(game.clone(), participants()).await.unwrap();
    let mut tasks = JoinSet::new();
    let barrier = Arc::new(Barrier::new(32));
    for index in 0..32 {
        let service = service.clone();
        let game = game.clone();
        let barrier = barrier.clone();
        tasks.spawn(async move {
            barrier.wait().await;
            service
                .make_move(
                    &game,
                    movement(
                        &format!("request-{index}"),
                        0,
                        Player::Black,
                        if index % 2 == 0 { "7g7f" } else { "2g2f" },
                    ),
                )
                .await
        });
    }
    let mut accepted = 0;
    while let Some(result) = tasks.join_next().await {
        match result.unwrap() {
            Ok(receipt) => {
                accepted += 1;
                assert_eq!(receipt.seq, 1);
            }
            Err(error) => assert_eq!(
                error,
                Error::SequenceMismatch {
                    expected: 0,
                    actual: 1
                }
            ),
        }
    }
    assert_eq!(accepted, 1);
    assert_eq!(service.snapshot(&game).await.unwrap().seq, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_identical_retries_all_receive_original_acceptance() {
    let (_database, service) = setup().await;
    let game = id("retry");
    service.create(game.clone(), participants()).await.unwrap();
    let command = movement("same-request", 0, Player::Black, "7g7f");
    let mut tasks = JoinSet::new();
    let barrier = Arc::new(Barrier::new(32));
    for _ in 0..32 {
        let (service, game, command, barrier) = (
            service.clone(),
            game.clone(),
            command.clone(),
            barrier.clone(),
        );
        tasks.spawn(async move {
            barrier.wait().await;
            service.make_move(&game, command).await
        });
    }
    while let Some(result) = tasks.join_next().await {
        let acceptance = result.unwrap().unwrap();
        assert_eq!(acceptance.seq, 1);
        assert_eq!(acceptance.result, None);
    }
    assert_eq!(service.snapshot(&game).await.unwrap().seq, 1);
}

#[tokio::test]
async fn every_rejection_preserves_snapshot_and_does_not_reserve_id() {
    let (_database, service) = setup().await;
    let game = id("rejection");
    let before = service.create(game.clone(), participants()).await.unwrap();
    for (command, expected) in [
        (
            movement("correct-me", 2, Player::Black, "7g7f"),
            Error::SequenceMismatch {
                expected: 2,
                actual: 0,
            },
        ),
        (
            movement("correct-me", 0, Player::White, "3c3d"),
            Error::Domain(CommandError::WrongTurn),
        ),
        (
            movement("correct-me", 0, Player::Black, "7g7e"),
            Error::Domain(CommandError::IllegalMove),
        ),
    ] {
        assert_eq!(service.make_move(&game, command).await, Err(expected));
        assert_eq!(service.snapshot(&game).await.unwrap(), before);
    }
    let receipt = service
        .resign(&game, resignation("correct-me", 0, Player::White))
        .await
        .unwrap();
    assert_eq!(receipt.seq, 1);
    assert_eq!(receipt.result.unwrap().winner, Player::Black);
    let finished = service.snapshot(&game).await.unwrap();
    assert_eq!(finished.position, before.position);
    assert_eq!(finished.turn, before.turn);
    assert!(finished.legal_moves.is_empty());
    assert_eq!(
        service
            .make_move(&game, movement("after", 1, Player::Black, "7g7f"))
            .await,
        Err(Error::Domain(CommandError::GameFinished))
    );
    assert_eq!(
        service
            .resign(&game, resignation("after", 1, Player::Black))
            .await,
        Err(Error::Domain(CommandError::GameFinished))
    );
    assert_eq!(service.snapshot(&game).await.unwrap(), finished);
}

#[tokio::test]
async fn replay_precedes_stale_and_finished_checks_and_fingerprint_is_exact() {
    let (_database, service) = setup().await;
    let game = id("ledger");
    service.create(game.clone(), participants()).await.unwrap();
    let first = movement("first", 0, Player::Black, "7g7f");
    let receipt = service.make_move(&game, first.clone()).await.unwrap();
    service
        .make_move(&game, movement("second", 1, Player::White, "3c3d"))
        .await
        .unwrap();
    let resign_command = resignation("resign", 2, Player::White);
    let finish_receipt = service.resign(&game, resign_command.clone()).await.unwrap();
    assert_eq!(
        finish_receipt.result.unwrap().reason,
        FinishReason::Resignation
    );
    let finished = service.snapshot(&game).await.unwrap();
    assert_eq!(service.make_move(&game, first).await, Ok(receipt));
    assert_eq!(
        service.resign(&game, resign_command).await,
        Ok(finish_receipt)
    );
    for changed in [
        movement("first", 1, Player::Black, "7g7f"),
        movement("first", 0, Player::White, "7g7f"),
        movement("first", 0, Player::Black, "2g2f"),
    ] {
        assert_eq!(
            service.make_move(&game, changed).await,
            Err(Error::RequestConflict)
        );
    }
    assert_eq!(
        service
            .resign(&game, resignation("first", 0, Player::Black))
            .await,
        Err(Error::RequestConflict)
    );
    assert_eq!(
        service
            .make_move(&game, movement("resign", 2, Player::White, "3c3d"))
            .await,
        Err(Error::RequestConflict)
    );
    assert_eq!(service.snapshot(&game).await.unwrap(), finished);
}

#[tokio::test]
async fn games_and_independent_services_have_separate_id_namespaces() {
    let (_database, service) = setup().await;
    let (_other_database, other) = setup().await;
    let (a, b) = (id("a"), id("b"));
    let initial_a = service.create(a.clone(), participants()).await.unwrap();
    let initial_b = service.create(b.clone(), participants()).await.unwrap();
    other.create(a.clone(), participants()).await.unwrap();
    service
        .make_move(&a, movement("shared", 0, Player::Black, "7g7f"))
        .await
        .unwrap();
    assert_eq!(service.snapshot(&b).await.unwrap(), initial_b);
    assert_eq!(other.snapshot(&a).await.unwrap(), initial_a);
    service
        .resign(&b, resignation("shared", 0, Player::White))
        .await
        .unwrap();
    assert!(service.snapshot(&a).await.unwrap().result.is_none());
    assert_eq!(service.snapshot(&id("missing")).await, Err(Error::NotFound));
    assert_eq!(
        service
            .make_move(&id("missing"), movement("m", 0, Player::Black, "7g7f"))
            .await,
        Err(Error::NotFound)
    );
    assert_eq!(
        service
            .resign(&id("missing"), resignation("r", 0, Player::White))
            .await,
        Err(Error::NotFound)
    );
}

fn user(side: Player) -> UserId {
    UserId::new(match side {
        Player::Black => "black",
        Player::White => "white",
    })
    .unwrap()
}
fn participants() -> Participants {
    Participants::new(user(Player::Black), user(Player::White)).unwrap()
}
async fn setup() -> (TestDatabase, GameService) {
    let database = TestDatabase::new().await;
    let store = Arc::new(
        game_persistence::SeaOrmStore::connect(database.url())
            .await
            .unwrap(),
    );
    (database, GameService::new(store))
}
