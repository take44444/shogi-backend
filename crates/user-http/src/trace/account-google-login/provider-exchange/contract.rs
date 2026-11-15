use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "exchange_code は保存済み PKCE verifier を token endpoint に送り、取得した ID token を verify_response に渡す。",
    source: trace::Source {
        path: "crates/user-http/src/google.rs",
        needle: "async fn exchange_code("
    },
    gap: Some(
        "GoogleProvider::exchange の再ディスカバリー、実 Google 通信、HTTPS 専用クライアントのタイムアウトとリダイレクト拒否をこのテストは通らない。"
    ),
});
