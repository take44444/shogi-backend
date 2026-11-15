use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "tick serializes durable admission, locks selected tickets and rating states, rechecks compatibility after waits and atomically creates game/rating reservations and both Playing tickets.",
    source: trace::Source {
        path: "crates/game-persistence/src/matchmaking.rs",
        needle: "async fn tick(&self, pool: Pool) -> Result<Option<GameId>, MError> {"
    },
    gap: Some(
        "The selected tests do not compare both ticket side bindings with the committed game's participant bindings. The executable test checks snapshot users, but does not compare ticket-side equality."
    ),
});
