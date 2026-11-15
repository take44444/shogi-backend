use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "event_json emits stable event identity with game/sequence/kind and move/result fields, omitting internal user and request IDs.",
    source: trace::Source {
        path: "crates/realtime-gateway/src/cursor.rs",
        needle: "pub(crate) fn event_json(notification: &Notification) -> Value {"
    },
    gap: Some(
        "No direct assertion proves the hex event_id encoding or uniqueness across all game identifiers."
    ),
});
