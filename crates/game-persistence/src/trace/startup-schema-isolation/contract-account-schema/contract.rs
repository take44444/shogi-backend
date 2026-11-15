use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "PostgresAccounts::connect serializes schema initialization/validation and rejects missing or changed required account definitions and ledger while preserving compatible data.",
    source: trace::Source {
        path: "crates/user-persistence/src/lib.rs",
        needle: "    pub async fn connect(url: &str) -> Result<Self, Error> {"
    },
    gap: Some(
        "These selected mutations are not every DDL drift. Account runtime/provider startup and signal shutdown are not tested by persistence cases."
    ),
});
