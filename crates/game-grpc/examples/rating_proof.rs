use game_grpc::proto::{
    CreateRatedGameRequest, GetSnapshotRequest, RatingPool, ResignRequest,
    game_service_client::GameServiceClient,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("usage: rating_proof GRPC_ENDPOINT GAME_PREFIX".into());
    }
    let mut client = GameServiceClient::connect(args[0].clone()).await?;
    let prefix = &args[1];
    let created = client
        .create_rated_game(CreateRatedGameRequest {
            game_id: prefix.clone(),
            black_user_id: format!("{prefix}-black"),
            white_user_id: format!("{prefix}-white"),
            pool: RatingPool::TenMinute30 as i32,
        })
        .await?
        .into_inner();
    assert_eq!(created.seq, 0);
    let identity = client
        .get_rated_game(GetSnapshotRequest {
            game_id: prefix.clone(),
        })
        .await?
        .into_inner();
    assert_eq!(identity.pool, RatingPool::TenMinute30 as i32);
    let result = client
        .resign(ResignRequest {
            game_id: prefix.clone(),
            request_id: format!("{prefix}-finish"),
            expected_seq: Some(0),
            user_id: format!("{prefix}-white"),
        })
        .await?
        .into_inner();
    assert_eq!(
        result.result.unwrap().winner,
        game_grpc::proto::Player::Black as i32
    );
    println!("finished {prefix}, black={prefix}-black, white={prefix}-white");
    Ok(())
}
