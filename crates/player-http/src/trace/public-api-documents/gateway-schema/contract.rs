use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "ゲートウェイは、注釈を付けたマッチングのパスと二つの Cookie セキュリティ方式を、オフライン OpenAPI 取得関数から出力する。",
    source: trace::Source {
        path: "crates/realtime-gateway/src/matchmaking.rs",
        needle: "pub(crate) fn openapi() -> utoipa::openapi::OpenApi {"
    },
    gap: Some(
        "このテストはオフラインの OpenAPI を検査し、実際の HTTP レスポンスの全フィールドとの一致は検証しない。"
    ),
});
