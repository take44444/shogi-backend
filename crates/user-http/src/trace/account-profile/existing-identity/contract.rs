use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "PostgresAccounts::login は既存 issuer・subject の Profile を再利用し、provider の initial_name で既存表示名を置き換えない。",
    source: trace::Source {
        path: "crates/user-persistence/src/lib.rs",
        needle: "    async fn login("
    },
    gap: None,
});
