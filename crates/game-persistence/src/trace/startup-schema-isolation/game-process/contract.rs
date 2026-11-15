use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The game binary chooses its address from the first argument, GAME_SERVER_ADDR or loopback default, connects the writer schema before serving gRPC, and signals its matcher to stop after server shutdown.",
    source: trace::Source {
        path: "apps/game-server/src/main.rs",
        needle: "async fn main() -> Result<(), Box<dyn std::error::Error>> {"
    },
    gap: Some(
        "No mapped test starts this binary to assert address precedence, malformed configuration, dependency failure, Ctrl-C or matcher teardown. Matcher behavior belongs to matchmaking; writer adapter tests do not prove process lifecycle."
    ),
});
