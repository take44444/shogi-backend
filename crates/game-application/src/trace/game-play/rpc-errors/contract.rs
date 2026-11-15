use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Application/domain errors map to stable gRPC classes, including FailedPrecondition for ordinary rejection, PermissionDenied for nonparticipant, Aborted with actual-seq for stale sequence and Internal for rules/infrastructure-internal errors.",
    source: trace::Source {
        path: "crates/game-grpc/src/conversion.rs",
        needle: "pub(crate) fn application_status(error: ApplicationError) -> Status {"
    },
    gap: None,
});
