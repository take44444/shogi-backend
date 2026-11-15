use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "公開 OpenAPI にレート取得と、両プール・必須フィールド・null 許容 rating のスキーマを含める。",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "fn api_router() -> OpenApiRouter<App> {"
    },
    gap: None,
});
