use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "RatingWorker routes invalid input and corrupt application to quarantine, retries an unavailable apply once to reconcile uncertain commits, and reports the repository outcome.",
    source: trace::Source {
        path: "crates/game-application/src/rating.rs",
        needle: "pub async fn tick(&self, owner: &WorkerId) -> Result<TickOutcome, RatingWorkerError> {"
    },
    gap: Some(
        "Selected error paths are asserted; there is no independent complete branch matrix or direct dedicated constructor test for WorkerId, WorkerPolicy and ClaimedRatingJob."
    ),
});
