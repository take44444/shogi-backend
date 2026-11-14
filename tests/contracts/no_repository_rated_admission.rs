use game_application::{GameId, GameRepository, Participants};

async fn bypass(target: &dyn GameRepository, id: GameId, players: Participants) {
    target
        .create_rated_game(id, players, game_application::rating::Pool::TenMinute30)
        .await;
}
