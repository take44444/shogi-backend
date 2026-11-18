use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "gateway-session",
    consumer: "realtime-gateway",
    provider: "user-http",
    interface: trace::Interface::Http,
    statement: "セッション Cookie を検証してプロフィールを返す。失効・ログアウト済みのセッションを拒否し、認証情報を応答に公開しない。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "async fn me(State(app): State<App>, headers: HeaderMap) -> Result<Json<ProfileBody>, HttpError> {"
    },
    call: Some(trace::Source {
        path: "crates/realtime-gateway/src/auth.rs",
        needle: "    pub(crate) async fn authenticate("
    }),
    implementation: Some(trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "async fn me(State(app): State<App>, headers: HeaderMap) -> Result<Json<ProfileBody>, HttpError> {"
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
