use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The player, user and matchmaking example entrypoints serialize their library document as pretty JSON.",
    source: trace::Source {
        path: "crates/player-http/examples/player-openapi.rs",
        needle: "fn main() -> Result<(), Box<dyn std::error::Error>> {"
    },
    gap: Some(
        "No mapped test launches the example processes or checks their stdout, the shell generation script or published files. Equivalent examples are crates/user-http/examples/user-openapi.rs and crates/realtime-gateway/examples/matchmaking-openapi.rs."
    ),
});
