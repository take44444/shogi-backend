use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "GoogleProvider::authorize は state・nonce とランダムな S256 PKCE challenge/verifier を生成し、openid・profile の認可 URL を返す。",
    source: trace::Source {
        path: "crates/user-http/src/google.rs",
        needle: "impl IdentityProvider for GoogleProvider {"
    },
    gap: None,
});
