use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "AuthClient::authenticate はセッション Cookie を user HTTP の /me に送り、200 の JSON にある型付き UserId を本人識別として使う。401 は認証失敗、他の異常は unavailable として扱う。",
    source: trace::Source {
        path: "crates/realtime-gateway/src/auth.rs",
        needle: "    pub(crate) async fn authenticate("
    },
    gap: Some(
        "user HTTP のリダイレクト、非 200/401、壊れた・大きすぎる JSON、通信失敗の分岐をこの記録のテストは直接確認していない。"
    ),
});
