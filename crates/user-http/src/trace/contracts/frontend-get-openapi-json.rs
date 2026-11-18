use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "frontend-get-openapi-json",
    consumer: "frontend",
    provider: "user-http",
    interface: trace::Interface::Http,
    statement: "GET /openapi.json は公開された入力・認証条件を検証し、機能の保証に従う応答・エラー・ヘッダーを返す。内部の保存情報や資格情報を公開しない。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {"
    },
    call: None,
    implementation: Some(trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {"
    }),
    binding: None,
    requires: &[],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
