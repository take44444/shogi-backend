use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "GET /me は Cookie による認証結果を ProfileBody の id・display_name・created_at に変換する。",
    source: trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "impl From<Profile> for ProfileBody {"
    },
    gap: Some(
        "created_at の値・型・単位を runtime で独立に確認する assertion はなく、変更前後の全体比較に含まれるだけである。Cookie の認証・期限・重複は account-session が扱う。"
    ),
});
