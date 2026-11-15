use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Reload clears local state before reading/validating durable history; failed reconciliation leaves it unavailable until a later refresh can restore it.",
    source: trace::Source {
        path: "crates/game-application/src/actor.rs",
        needle: "async fn reload(&mut self) -> Result<(), ApplicationError> {"
    },
    gap: Some(
        "The fault adapter controls representative outcome paths; arbitrary process/crash interleavings are not proven."
    ),
});
