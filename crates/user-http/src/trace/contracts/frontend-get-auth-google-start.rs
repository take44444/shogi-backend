use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "frontend-get-auth-google-start",
    consumer: "frontend",
    provider: "user-http",
    interface: trace::Interface::Http,
    statement: "GET /auth/google/start は公開された入力・認証条件を検証し、機能の保証に従う応答・エラー・ヘッダーを返す。内部の保存情報や資格情報を公開しない。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "async fn start(State(app): State<App>) -> Result<Response, HttpError> {"
    },
    call: None,
    implementation: Some(trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "async fn start(State(app): State<App>) -> Result<Response, HttpError> {"
    }),
    binding: None,
    requires: &[trace::Dependency {
        contract: "user-application/account-store",
        reason: "提供元はこの直接の依存先を利用して、上位の応答と状態の保証を満たす。"
    }],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
