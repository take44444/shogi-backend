use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "GetSnapshotForViewer and CheckViewAccess validate game/viewer IDs, forward caller viewer claims and map application errors without authenticating the claim.",
    source: trace::Source {
        path: "crates/game-grpc/src/service.rs",
        needle: "async fn get_snapshot_for_viewer("
    },
    gap: Some(
        "Malformed viewer/game ID boundaries are not individually asserted on both viewer RPCs."
    ),
});
