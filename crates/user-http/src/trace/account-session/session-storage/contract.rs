use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "auth_query は token_hash と expires_at > now で session と account を結合する。HTTP が保存するセッションの識別値は生の Cookie ではなく SHA-256 となる。",
    source: trace::Source {
        path: "crates/user-persistence/src/lib.rs",
        needle: "fn auth_query(hash:"
    },
    gap: Some(
        "破損プロフィールの decode と保存層障害の HTTP 503 変換はこの記録のテストで直接確認していない。"
    ),
});
