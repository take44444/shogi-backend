use crate::trace_support::{self as trace, Contract};
trace::contract!(Contract {
    statement: "The binary acquires queue ownership, drops its runtime before dropping the queue, and runs an idle-delayed Ctrl-C-aware dispatch loop.",
    source: trace::Source {
        path: "apps/outbox-publisher/src/main.rs",
        needle: "async fn run(queue: &mut PostgresOutbox, valkey: &str) -> Result<(), Error> {"
    },
    gap: Some(
        "No mapped test starts this binary or asserts its environment parsing, 100 ms idle delay, error exit, Ctrl-C handling, runtime teardown or process restart behavior."
    )
});
