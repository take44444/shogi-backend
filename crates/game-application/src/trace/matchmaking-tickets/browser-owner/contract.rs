use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Browser handlers derive owner from session authentication and check exact Host, required mutation Origin and optional same-site metadata before typed admission.",
    source: trace::Source {
        path: "crates/realtime-gateway/src/matchmaking.rs",
        needle: "async fn authenticate("
    },
    gap: Some(
        "No dedicated assertion covers every malformed UUID, DELETE body rejection, saturated HTTP permit or authentication/backend timeout branch."
    ),
});
