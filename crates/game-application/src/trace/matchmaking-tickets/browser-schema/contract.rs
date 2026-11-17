use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "生成する組合せ用 OpenAPI は、所有者に限定した六つの操作と Cookie 認証の選択肢を含む。",
    source: trace::Source {
        path: "crates/realtime-gateway/src/matchmaking.rs",
        needle: "pub(crate) fn openapi() -> utoipa::openapi::OpenApi {"
    },
    gap: None,
});
