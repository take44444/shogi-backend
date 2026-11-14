use game_application::{GameId, GameService, Participants};

async fn bypass(service: &GameService, id: GameId, players: Participants) {
    service.create(id, players).await;
}
