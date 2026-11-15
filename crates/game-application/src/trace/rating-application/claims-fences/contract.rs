use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The durable repository acquires eligible jobs with skip-locked behavior and checks owner, generation and live lease before processing and again when completing atomic application after state-lock waits.",
    source: trace::Source {
        path: "crates/game-persistence/src/rating.rs",
        needle: "impl RatingJobRepository for SeaOrmRatingJobs {"
    },
    gap: None,
});
