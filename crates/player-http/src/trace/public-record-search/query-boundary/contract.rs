use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "RawSearch parses typed fields and rejects invalid, unknown, or repeated query values before repository access.",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "fn parse(self) -> Result<Search, Error> {"
    },
    gap: None,
});
