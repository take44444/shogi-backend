use game_application::{GameId, GameRepository, Participants};

async fn bypass(target: &dyn GameRepository, id: GameId, players: Participants) {
    target.create_game(id, players).await;
}
