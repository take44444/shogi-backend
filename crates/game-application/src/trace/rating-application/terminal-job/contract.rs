use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The game commit atomically enqueues a job only for a terminal rated game and keeps one job under receipt replay.",
    source: trace::Source {
        path: "crates/game-persistence/src/lib.rs",
        needle: "async fn commit_change(&self, change: &StagedGameChange) -> Result<(), Error> {"
    },
    gap: None,
});
