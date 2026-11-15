use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "PostgresAccounts::login は issuer・subject のロック下で既存プロフィールを再利用するかアカウントと外部 ID を作成し、セッション挿入と一緒にコミットする。",
    source: trace::Source {
        path: "crates/user-persistence/src/lib.rs",
        needle: "    async fn login("
    },
    gap: Some(
        "issuer だけが異なる場合の分離や、コミット時失敗などすべてのトランザクション障害は列挙していない。"
    ),
});
