use game_application::{GameId, GameService, Participants};

async fn bypass(target: &GameService, id: GameId, players: Participants) {
    target
        .create_rated(id, players, game_application::rating::Pool::TenMinute30)
        .await;
}
