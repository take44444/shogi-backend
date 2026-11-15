use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Resign validates game/request/user IDs and required expected sequence, then delegates the authenticated-at-gateway participant claim to the application.",
    source: trace::Source {
        path: "crates/game-grpc/src/service.rs",
        needle: "async fn resign("
    },
    gap: None,
});
