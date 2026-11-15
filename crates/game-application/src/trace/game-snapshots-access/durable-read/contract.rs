use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "PostgreSQL joins game metadata and ordered receipts in one coherent read; viewer read applies its visibility filter to that view.",
    source: trace::Source {
        path: "crates/game-persistence/src/lib.rs",
        needle: "async fn load_view("
    },
    gap: Some(
        "Concurrent ordinary joined reads are asserted; concurrent viewer visibility/history changes are not independently asserted."
    ),
});
