use super::*;
use tonic::Code;

#[test]
fn infrastructure_and_rules_errors_have_stable_status_codes() {
    for (error, expected) in [
        (ApplicationError::AlreadyExists, Code::AlreadyExists),
        (ApplicationError::RequestConflict, Code::AlreadyExists),
        (ApplicationError::NotFound, Code::NotFound),
        (ApplicationError::Unavailable, Code::Unavailable),
        (ApplicationError::Internal, Code::Internal),
        (ApplicationError::SequenceExhausted, Code::Internal),
        (
            ApplicationError::Domain(CommandError::RulesFailure),
            Code::Internal,
        ),
        (
            ApplicationError::Domain(CommandError::WrongTurn),
            Code::FailedPrecondition,
        ),
        (
            ApplicationError::Domain(CommandError::IllegalMove),
            Code::FailedPrecondition,
        ),
        (
            ApplicationError::Domain(CommandError::GameFinished),
            Code::FailedPrecondition,
        ),
    ] {
        assert_eq!(application_status(error).code(), expected);
    }
}

#[test]
fn checkmate_result_preserves_winner_and_reason() {
    let result = proto::GameResult::from(GameResult {
        winner: Player::White,
        reason: FinishReason::Checkmate,
    });
    assert_eq!(result.winner, proto::Player::White as i32);
    assert_eq!(result.reason, proto::FinishReason::Checkmate as i32);
}
