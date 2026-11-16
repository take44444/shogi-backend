use super::*;
use tonic::Code;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "エラーからStatusへの直接変換。ネットワーク、データベース、基盤障害の注入は使わない。",
        reason: "直接変換の検証で、基盤障害、破損、ルール処理の失敗、連番の上限到達、連番の不一致に対応するステータスを区別する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "終局結果の列挙型を直接変換する。ネットワーク経由で詰みの手順を進めるものではない。",
        reason: "詰みの結果を変換しても、勝者と終局理由を保持する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
    fn checkmate_result_preserves_winner_and_reason() {
        let result = proto::GameResult::from(GameResult {
            winner: Player::White,
            reason: FinishReason::Checkmate,
        });
        assert_eq!(result.winner, proto::Player::White as i32);
        assert_eq!(result.reason, proto::FinishReason::Checkmate as i32);
    }
}
