use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Unrated rating is effectively 1500; allowed difference starts at 100 and increases by 50 per complete 30 seconds to 400, using wide arithmetic.",
    source: trace::Source {
        path: "crates/game-application/src/matchmaking.rs",
        needle: "pub fn window(enqueued_at: i64, now: i64) -> i128 {"
    },
    gap: None,
});
