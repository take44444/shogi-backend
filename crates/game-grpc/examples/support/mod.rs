use game_application::matchmaking::TicketId;
use game_grpc::proto::{self, game_service_client::GameServiceClient};
use tonic::{Code, transport::Channel};

fn uncertain(status: &tonic::Status) -> bool {
    matches!(
        status.code(),
        Code::Unavailable | Code::DeadlineExceeded | Code::Cancelled | Code::Unknown
    )
}

pub async fn pair(
    client: &mut GameServiceClient<Channel>,
    users: [&str; 2],
) -> Result<proto::Snapshot, Box<dyn std::error::Error>> {
    let tickets = [TicketId::new().to_string(), TicketId::new().to_string()];
    for (user, ticket) in users.iter().zip(&tickets) {
        let request = proto::JoinMatchmakingRequest {
            user_id: (*user).into(),
            ticket_id: ticket.clone(),
            pool: proto::RatingPool::TenMinute30 as i32,
        };
        let response = tokio::time::timeout(std::time::Duration::from_secs(15), async {
            loop {
                match client.join_matchmaking(request.clone()).await {
                    Ok(response) => break Ok(response.into_inner()),
                    Err(status) if uncertain(&status) => {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                    Err(status) => break Err(status),
                }
            }
        })
        .await??;
        if response.ticket.as_ref().is_none_or(|ticket| {
            !matches!(ticket.state.as_str(), "waiting" | "matched" | "playing")
        }) {
            return Err(format!(
                "admission stopped in state {:?}; use fresh users after rating completion",
                response.ticket.as_ref().map(|ticket| ticket.state.as_str())
            )
            .into());
        }
    }
    let mut sequences = [1, 1];
    let mut confirmed = [false, false];
    for _ in 0..150 {
        for index in 0..2 {
            let mut ticket = match client
                .heartbeat_matchmaking(proto::HeartbeatMatchmakingRequest {
                    user_id: users[index].into(),
                    ticket_id: tickets[index].clone(),
                    sequence: sequences[index],
                })
                .await
            {
                Ok(response) => {
                    sequences[index] += 1;
                    response.into_inner()
                }
                Err(status) if uncertain(&status) => continue,
                Err(status) => return Err(status.into()),
            };
            if ticket.state == "matched" && !confirmed[index] {
                let request = proto::MatchmakingRequest {
                    user_id: users[index].into(),
                    ticket_id: tickets[index].clone(),
                };
                ticket = tokio::time::timeout(std::time::Duration::from_secs(15), async {
                    loop {
                        match client.ready_matchmaking(request.clone()).await {
                            Ok(response) => break Ok(response.into_inner()),
                            Err(status) if uncertain(&status) => {
                                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            }
                            Err(status) => break Err(status),
                        }
                    }
                })
                .await??;
                confirmed[index] = true;
            }
            match ticket.state.as_str() {
                "waiting" | "matched" => {}
                "playing" => {
                    let game_id = ticket.game_id.ok_or("playing ticket has no game ID")?;
                    return Ok(client
                        .get_snapshot(proto::GetSnapshotRequest { game_id })
                        .await?
                        .into_inner());
                }
                state => {
                    return Err(format!("matchmaking stopped in terminal state {state}").into());
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Err("matchmaking did not form a pair".into())
}
