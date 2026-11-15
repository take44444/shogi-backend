use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The worker binary parses bounded policy and poll values, starts a durable repository and runs either polling or explicit retry-quarantined mode before closing it.",
    source: trace::Source {
        path: "apps/rating-worker-server/src/main.rs",
        needle: "async fn main() -> Result<(), Box<dyn std::error::Error>> {"
    },
    gap: Some(
        "No mapped test starts this CLI, checks actual argument/environment parsing, zero-poll rejection, Ctrl-C, poll timing or repository close on process shutdown."
    ),
});
