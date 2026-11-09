use game_application::{GameRepository, Participants, UserId};
use game_record_application::{FinishReason, GameId, GameResult, Metadata, MovePly, OperationSequence, Player, PreparedFinishTime, Rule};
use game_record_persistence::PostgresGameRecords;

fn public_values(black: UserId, white: UserId) {
    let participants = Participants::new(black, white).unwrap();
    let _ = Metadata::new(
        GameId::new("game").unwrap(),
        participants.black().clone(), participants.white().clone(), Rule::Standard,
        GameResult { winner: Player::Black, reason: FinishReason::Resignation },
        PreparedFinishTime::try_from(0i64).unwrap(),
        OperationSequence::try_from(1u64).unwrap(), MovePly::try_from(0u64).unwrap(),
    ).unwrap();
}

fn supported_results(reason: FinishReason) -> bool {
    match reason {
        FinishReason::Checkmate => true,
        FinishReason::Resignation => false,
    }
}

fn supported_rules(rule: Rule) {
    match rule { Rule::Standard => {} }
}

fn accepted_interfaces(_: &dyn GameRepository, _: Option<PostgresGameRecords>) {}
