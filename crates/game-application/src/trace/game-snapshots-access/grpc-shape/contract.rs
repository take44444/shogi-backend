use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Snapshot conversion includes participant bindings, sequence, occupied board squares, both players/all hand kinds, turn/check/result and legal USI moves.",
    source: trace::Source {
        path: "crates/game-grpc/src/conversion.rs",
        needle: "impl From<Snapshot> for proto::Snapshot {"
    },
    gap: Some(
        "The selected transport test does not compare the complete board/hand ordering or every piece/enum conversion branch."
    ),
});
