use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The public reader binary parses PLAYER_BIND_ADDR, connects PLAYER_GAME_DATABASE_URL through the validating read adapter before binding, shares that reader between records and ratings, and closes it after graceful Ctrl-C shutdown.",
    source: trace::Source {
        path: "apps/player-server/src/main.rs",
        needle: "async fn main() -> Result<(), Box<dyn std::error::Error>> {"
    },
    gap: Some(
        "No mapped test starts this binary to assert environment defaults, bind parsing, rejected schema before listening, Ctrl-C or reader close. In-process HTTP and database adapter tests do not assert this executable wiring."
    ),
});
