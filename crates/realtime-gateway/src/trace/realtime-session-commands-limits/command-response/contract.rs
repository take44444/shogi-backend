use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "command_reply maps acceptance and gRPC codes to accepted, invalid_input, not_participant, request_conflict, stale_seq/actual_seq, rejected or outcome_unknown; inaccessible/corrupt/internal upstream outcomes close.",
    source: trace::Source {
        path: "crates/realtime-gateway/src/game.rs",
        needle: "pub(crate) fn command_reply(id: &str, result: Result<proto::Acceptance, Status>) -> CommandOutcome {"
    },
    gap: Some(
        "No selected gateway test directly asserts stale_seq metadata, request_conflict, rejected, outcome_unknown or all upstream status/invalid-result close mappings."
    ),
});
