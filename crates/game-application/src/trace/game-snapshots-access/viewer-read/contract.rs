use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "snapshot_for_viewer uses one repository access/history view, rejects access or game-identity mismatch and reconstructs the owned snapshot.",
    source: trace::Source {
        path: "crates/game-application/src/lib.rs",
        needle: "pub async fn snapshot_for_viewer("
    },
    gap: Some(
        "No concurrent privacy plus history mutation test independently establishes the complete coherent viewer-read contract."
    ),
});
