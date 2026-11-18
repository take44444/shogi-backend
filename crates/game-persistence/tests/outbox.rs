#[path = "contracts/durable-event-dispatch/support.rs"]
mod outbox;
#[cfg(test)]
#[path = "../../../scripts/traceability_model.rs"]
mod trace_support;
