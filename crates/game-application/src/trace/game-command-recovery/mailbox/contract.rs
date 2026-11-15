use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The per-game facade uses a 64-command bounded channel; actor execution outlives dropped reply receivers and drains closed queues.",
    source: trace::Source {
        path: "crates/game-application/src/lib.rs",
        needle: "const MAILBOX_CAPACITY: usize = 64;"
    },
    gap: Some(
        "64 bounds queued slots, excluding in-progress work and waiting senders. No global known-game actor registry bound or fairness guarantee is established."
    ),
});
