use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "ユーザーサービスの文書ルートは、保存層や外部認証プロバイダーを呼ばずにオフラインの OpenAPI を返し、非公開応答用のヘッダーを付ける。",
    source: trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "pub fn openapi() -> utoipa::openapi::OpenApi {"
    },
    gap: None,
});
