use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Apply validates frozen/current state and fencing, updates both players and the application ledger/job/reservations atomically, and checks completed replay without modifying later reservations.",
    source: trace::Source {
        path: "crates/game-persistence/src/rating.rs",
        needle: "    async fn apply("
    },
    gap: None,
});
