use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "AuthClientは正しい形式のCookieを1件だけ受け入れ、応答サイズと所要時間を制限してユーザーサービスを呼ぶ。再確認では、返されたユーザーがセッションのユーザーと同じであることを求める。",
    source: trace::Source {
        path: "crates/realtime-gateway/src/auth.rs",
        needle: "pub(crate) async fn authenticate(&self, cookie: &str) -> Result<UserId, AuthError> {"
    },
    gap: Some(
        "認証サービスの停止・リダイレクト・過大なプロフィール、返されたユーザーの不一致、Cookieのエンコードの全分岐を扱う専用の検証はない。"
    ),
});
