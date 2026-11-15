crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct error-to-Status conversion; no network, database or injected infrastructure failure.",
        reason: "Direct conversion assertions distinguish infrastructure, corruption, rules failure, sequence exhaustion and sequence mismatch status classes.",
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
