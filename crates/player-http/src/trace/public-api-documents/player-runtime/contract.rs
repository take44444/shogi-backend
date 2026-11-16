use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "公開対局サービスの文書ルートは、保存層を読み取らずに player_http::openapi と同じ生成済み OpenAPI を返す。",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "pub fn openapi() -> utoipa::openapi::OpenApi {"
    },
    gap: None,
});
