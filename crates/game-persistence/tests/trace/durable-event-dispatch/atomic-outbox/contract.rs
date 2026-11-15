use crate::trace_support::{self as trace, Contract};
trace::contract!(Contract {
    statement: "The repository commit persists the accepted receipt and frozen outbox rows in the same transaction and guards the game update with its expected sequence. Application receipt replay avoids a duplicate commit.",
    source: trace::Source {
        path: "crates/game-persistence/src/lib.rs",
        needle: "async fn commit_change(&self, change: &StagedGameChange) -> Result<(), Error> {"
    },
    gap: None
});
