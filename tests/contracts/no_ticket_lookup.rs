use game_application::{
    UserId,
    matchmaking::{MatchmakingRepository, TicketId},
};

async fn lookup(repository: &dyn MatchmakingRepository, user: &UserId, ticket: TicketId) {
    let _ = repository.ticket(user, ticket).await;
}
