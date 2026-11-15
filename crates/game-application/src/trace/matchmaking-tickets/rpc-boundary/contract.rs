use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The five matchmaking RPC methods parse owner/UUID, validate supported pool and positive heartbeat, delegate owner-scoped operations and serialize ticket states.",
    source: trace::Source {
        path: "crates/game-grpc/src/service.rs",
        needle: "async fn join_matchmaking("
    },
    gap: Some(
        "Malformed owner/UUID cases are not enumerated for every RPC; the dedicated TCP test does not assert a successful heartbeat. Browser tests assert successful heartbeat through RPC."
    ),
});
