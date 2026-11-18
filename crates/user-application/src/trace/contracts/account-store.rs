use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "account-store",
    consumer: "user-http",
    provider: "user-persistence",
    interface: trace::Interface::Port,
    statement: "ログイン試行を一度だけ消費し、本人識別に対するアカウントとセッションを原子的に保存する。認証・改名・ログアウトは保存済みセッションに従う。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/user-application/src/lib.rs",
        needle: "pub trait AccountRepository: Send + Sync {"
    },
    call: Some(trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "async fn callback("
    }),
    implementation: Some(trace::Source {
        path: "crates/user-persistence/src/lib.rs",
        needle: "    async fn login("
    }),
    binding: Some(trace::Source {
        path: "apps/user-server/src/main.rs",
        needle: "let app = user_http::router(store.clone(), provider, config.browser);"
    }),
    requires: &[],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
