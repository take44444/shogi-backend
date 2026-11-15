use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "HTTP アダプターは UserRatingSnapshot の両プールを JSON に変換し、未評価の null と 0 を保持する。",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "impl From<UserRatingSnapshot> for RatingsDto {"
    },
    gap: None,
});
