use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "ServerConfig::from_env requires nonempty public origin, PostgreSQL URL and Google credentials, bounds session lifetime to 60 through 2592000 seconds, parses the bind address and accepts only true/false development flags.",
    source: trace::Source {
        path: "crates/user-http/src/config.rs",
        needle: "pub fn from_env() -> Result<Self, ConfigError> {"
    },
    gap: Some(
        "BrowserConfig constructor tests do not call from_env. No mapped test mutates the environment to assert required variables, invalid URL prefixes, session lifetime bounds, bind parsing, development-flag parsing or defaults."
    ),
});
