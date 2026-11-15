use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "BrowserConfig validates scheme, loopback opt-in, credentials, path, query and fragment and derives callback_url from the normalized origin.",
    source: trace::Source {
        path: "crates/user-http/src/config.rs",
        needle: "pub fn new(origin: &str, allow_loopback_http: bool) -> Result<Self, ConfigError> {"
    },
    gap: Some(
        "ServerConfig::from_env requirements, PostgreSQL URL validation, session TTL, bind parsing and invalid development flags are not directly asserted by this constructor test."
    ),
});
