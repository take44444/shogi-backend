use crate::trace_support::{self as trace, Contract};
trace::contract!(Contract {
    statement: "The messaging adapter encodes schema-version-one JSON with explicit event identity and a channel derived from every game-ID byte.",
    source: trace::Source {
        path: "crates/game-messaging/src/lib.rs",
        needle: "fn encode(event: &CommittedEvent) -> Result<Vec<u8>, PublishError> {"
    },
    gap: Some(
        "Subscriber delivery, subscription lifecycle and notification rejection are mapped by realtime-gameplay rather than asserted by these publication examples."
    )
});
