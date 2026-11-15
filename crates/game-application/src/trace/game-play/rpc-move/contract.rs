use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "MakeMove validates game/request/user IDs, requires expected_seq, parses USI and forwards a typed application command.",
    source: trace::Source {
        path: "crates/game-grpc/src/service.rs",
        needle: "async fn make_move("
    },
    gap: None,
});
