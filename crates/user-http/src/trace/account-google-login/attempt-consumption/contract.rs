use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "PostgresAccounts::consume_attempt は state・browser の両ハッシュが一致し expires_at > now の行を DELETE RETURNING で一度だけ取得する。",
    source: trace::Source {
        path: "crates/user-persistence/src/lib.rs",
        needle: "    async fn consume_attempt("
    },
    gap: None,
});
