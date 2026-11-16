use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "BrowserConfig はスキーム、ループバックの明示指定、認証情報、パス、クエリー、フラグメントを検証し、正規化したオリジンから callback_url を導出する。",
    source: trace::Source {
        path: "crates/user-http/src/config.rs",
        needle: "pub fn new(origin: &str, allow_loopback_http: bool) -> Result<Self, ConfigError> {"
    },
    gap: Some(
        "ServerConfig::from_env の必須条件、PostgreSQL URL の検証、セッション TTL、待ち受けアドレスの解析、不正な開発用フラグは、このコンストラクターテストでは直接確認していない。"
    ),
});
