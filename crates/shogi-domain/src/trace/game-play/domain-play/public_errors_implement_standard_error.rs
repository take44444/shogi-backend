#[test]
fn public_errors_implement_standard_error() {
    fn error<T: std::error::Error>(value: T) {
        assert!(!value.to_string().is_empty());
    }
    error(InvalidSquare);
    error(ParseUsiError);
    for e in [
        CommandError::GameFinished,
        CommandError::WrongTurn,
        CommandError::IllegalMove,
        CommandError::RulesFailure,
    ] {
        error(e);
    }
}
