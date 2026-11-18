use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "google-identity",
    consumer: "user-http",
    provider: "google",
    interface: trace::Interface::External,
    statement: "保存した verifier でコードを交換し、署名・issuer・audience・nonce・期限などを検証した本人識別だけを受け入れる。",
    assumptions: &["呼出元は公開インターフェースの型と認証条件を満たす。"],
    limitations: &["下位契約の検証だけでは上位契約を証明しない。"],
    source: trace::Source {
        path: "crates/user-http/src/google.rs",
        needle: "async fn exchange_code("
    },
    call: Some(trace::Source {
        path: "crates/user-http/src/google.rs",
        needle: "async fn exchange_code("
    }),
    implementation: None,
    binding: None,
    requires: &[],
    gap: Some(
        "この境界の全入力・外部障害・競合順序は網羅しない。実外部サービスを使わないテストはその提供元の根拠に含めない。"
    )
});
