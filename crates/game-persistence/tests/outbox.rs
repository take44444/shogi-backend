#[path = "trace/durable-event-dispatch/support.rs"]
mod outbox;
#[cfg(test)]
#[path = "support/outbox_metadata.rs"]
mod trace_metadata;
#[cfg(test)]
#[path = "../../../scripts/traceability_model.rs"]
mod trace_support;
