use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "公開対局サービスのルートから生成する OpenAPI 3.1 は、パッケージの識別情報、過不足のない HTTP メソッドとパス、クエリーとエラーの制約、公開応答の構造を持つ。",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "fn api_router() -> OpenApiRouter<App> {"
    },
    gap: None,
});
