use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Game::play rejects finished or wrong-turn commands, delegates legality, and declares checkmate only when the next side is checked with no legal moves. Repetition/perpetual check are not adjudicated.",
    source: trace::Source {
        path: "crates/shogi-domain/src/lib.rs",
        needle: "pub fn play(&mut self, player: Player, mv: Move) -> Result<MoveOutcome, CommandError> {"
    },
    gap: Some(
        "No clock, timeout, entering-king or declaration adjudicator exists; tests do not enumerate every unsupported adjudication scenario."
    ),
});
