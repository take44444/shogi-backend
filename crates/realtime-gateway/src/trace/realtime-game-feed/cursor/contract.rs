use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "ConnectionState accepts only contiguous coherent committed facts, drops exact duplicates/old boundary events and checks same-sequence mate-finish agreement.",
    source: trace::Source {
        path: "crates/realtime-gateway/src/cursor.rs",
        needle: "pub(crate) fn accept(&mut self, notification: &Notification) -> Decision {"
    },
    gap: Some(
        "No real WebSocket checkmate pair assertion; direct cursor tests use synthetic committed notifications. The close helper allows either resync_required then close or immediate close."
    ),
});
