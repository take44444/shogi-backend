use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Public visibility or permanent participant membership permits access; CheckViewAccess uses metadata without loading history.",
    source: trace::Source {
        path: "crates/game-application/src/lib.rs",
        needle: "pub async fn check_view_access("
    },
    gap: Some(
        "No query-count assertion proves the access-only operation skips history, and every storage/access corruption branch is not enumerated."
    ),
});
