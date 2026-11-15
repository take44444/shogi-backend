use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The executable uses a one-second skipped-tick interval, alternates starting pools, tries at most 32 ticks and stops a batch on two empty pools or first failure.",
    source: trace::Source {
        path: "apps/game-server/src/main.rs",
        needle: "let matcher = tokio::spawn(async move {"
    },
    gap: Some(
        "The test ages DB timestamps instead of waiting 30 real seconds. It does not assert two-pool fairness, the 32-attempt cap, error logging/recovery or Ctrl-C shutdown."
    ),
});
