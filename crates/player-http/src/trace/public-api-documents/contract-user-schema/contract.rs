use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "ユーザーサービスのルートから生成する OpenAPI は、サービスの識別情報、過不足のない六つの操作、Cookie セキュリティ方式の選択肢、要求と応答のスキーマを持つ。",
    source: trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "fn api_router() -> OpenApiRouter<App> {"
    },
    gap: Some(
        "実行時の入力拒否の事例は、すべての応答種別や成功・エラーの全ヘッダーを網羅しない。スキーマの検査では外部認証プロバイダーを利用できることは確認しない。"
    ),
});
