use game_application::matchmaking::TicketId;
use game_grpc::proto::{self, game_service_client::GameServiceClient};
use tonic::transport::Channel;

pub async fn pair(
    client: &mut GameServiceClient<Channel>,
    users: [&str; 2],
) -> Result<proto::Snapshot, Box<dyn std::error::Error>> {
    let tickets = [TicketId::new().to_string(), TicketId::new().to_string()];
    for (user, ticket) in users.iter().zip(&tickets) {
        let response = client
            .join_matchmaking(proto::JoinMatchmakingRequest {
                user_id: (*user).into(),
                ticket_id: ticket.clone(),
                pool: proto::RatingPool::TenMinute30 as i32,
            })
            .await?
            .into_inner();
        if response
            .ticket
            .as_ref()
            .is_none_or(|t| t.state != "waiting")
        {
            return Err("admission refused; use fresh users after rating completion".into());
        }
    }
    for _ in 0..150 {
        let ticket = client
            .get_matchmaking_ticket(proto::MatchmakingRequest {
                user_id: users[0].into(),
                ticket_id: tickets[0].clone(),
            })
            .await?
            .into_inner();
        if let Some(game_id) = ticket.game_id {
            return Ok(client
                .get_snapshot(proto::GetSnapshotRequest { game_id })
                .await?
                .into_inner());
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Err("matchmaking did not form a pair".into())
}
