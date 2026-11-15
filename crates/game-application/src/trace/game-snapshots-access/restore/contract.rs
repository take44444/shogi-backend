use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Restoration validates receipt sequence, uniqueness, participant legality, acceptance and final metadata while replaying from the standard initial game.",
    source: trace::Source {
        path: "crates/game-application/src/actor.rs",
        needle: "fn restore(record: GameRecord) -> Result<Self, ApplicationError> {"
    },
    gap: None,
});
