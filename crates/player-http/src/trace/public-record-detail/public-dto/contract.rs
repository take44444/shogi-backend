use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "DetailDto emits metadata and ordered action DTOs without internal request, receipt, check or outbox fields.",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "impl From<Detail> for DetailDto {"
    },
    gap: Some(
        "This assertion covers one move and one resignation, not every move, drop and terminal-result encoding."
    ),
});
