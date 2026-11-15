crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. Wrong-turn/illegal rejections preserve board, hands, turn, check, result and legal moves.",
        reason: "Rejections compare all observable state.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn rejection_preserves_all_observable_state() {
    let mut game = Game::new();
    reject(&mut game, Player::White, "3c3d", CommandError::WrongTurn);
    for illegal in [
        "7g7e", "7f7e", "3c3d", "7g7g", "7g6g", "8h3c", "7g7f+", "P*5e", "5i5h+",
    ] {
        reject(&mut game, Player::Black, illegal, CommandError::IllegalMove);
    }
    play(&mut game, "7g7f");
    reject(&mut game, Player::Black, "2g2f", CommandError::WrongTurn);
    play(&mut game, "3c3d");
}
}
