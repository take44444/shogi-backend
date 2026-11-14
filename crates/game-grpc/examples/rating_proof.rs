mod support;
use game_grpc::proto::{ResignRequest, game_service_client::GameServiceClient};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("usage: rating_proof GRPC_ENDPOINT USER_PREFIX".into());
    }
    let mut client = GameServiceClient::connect(args[0].clone()).await?;
    let prefix = &args[1];
    let snapshot = support::pair(
        &mut client,
        [&format!("{prefix}-one"), &format!("{prefix}-two")],
    )
    .await?;
    let result = client
        .resign(ResignRequest {
            game_id: snapshot.game_id.clone(),
            request_id: format!("{prefix}-finish"),
            expected_seq: Some(0),
            user_id: snapshot.white_user_id.clone(),
        })
        .await?
        .into_inner();
    assert_eq!(
        result.result.unwrap().winner,
        game_grpc::proto::Player::Black as i32
    );
    println!(
        "finished {}, black={}, white={}",
        snapshot.game_id, snapshot.black_user_id, snapshot.white_user_id
    );
    Ok(())
}
