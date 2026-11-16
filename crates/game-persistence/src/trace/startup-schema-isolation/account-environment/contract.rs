use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "ServerConfig::from_env は空でない公開オリジン、PostgreSQL URL、Google 認証情報を必須とする。セッション有効期間を 60 秒から 2592000 秒に制限し、待ち受けアドレスを解析し、開発用フラグは true/false だけを受け入れる。",
    source: trace::Source {
        path: "crates/user-http/src/config.rs",
        needle: "pub fn from_env() -> Result<Self, ConfigError> {"
    },
    gap: Some(
        "BrowserConfig のコンストラクターテストは from_env を呼ばない。必須の環境変数、不正な URL プレフィックス、セッション有効期間の境界、待ち受けアドレスの解析、開発用フラグの解析と既定値を、環境を書き換えて検証する対応テストはない。"
    ),
});
