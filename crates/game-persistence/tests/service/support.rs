use super::*;

pub(super) fn id(value: &str) -> GameId {
    GameId::new(value).unwrap()
}
pub(super) fn movement(request: &str, seq: u64, player: Player, usi: &str) -> MakeMove {
    MakeMove {
        request_id: RequestId::new(request).unwrap(),
        expected_seq: seq,
        user_id: user(player),
        movement: usi.parse().unwrap(),
    }
}
pub(super) fn resignation(request: &str, seq: u64, player: Player) -> Resign {
    Resign {
        request_id: RequestId::new(request).unwrap(),
        expected_seq: seq,
        user_id: user(player),
    }
}

pub(super) fn user(side: Player) -> UserId {
    UserId::new(match side {
        Player::Black => "black",
        Player::White => "white",
    })
    .unwrap()
}
pub(super) fn participants() -> Participants {
    Participants::new(user(Player::Black), user(Player::White)).unwrap()
}
pub(super) async fn setup() -> (TestDatabase, pg_test_support::GameFixture) {
    let database = TestDatabase::new().await;
    let store = Arc::new(
        game_persistence::SeaOrmStore::connect(database.url())
            .await
            .unwrap(),
    );
    {
        let games = pg_test_support::GameFixture::new(store, database.url());
        (database, games)
    }
}
