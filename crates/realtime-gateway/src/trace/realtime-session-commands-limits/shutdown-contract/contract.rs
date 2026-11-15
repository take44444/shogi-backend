use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "shutdown stores a persistent stop signal; active connections watch it and upgrade refuses new work. wait_closed bounds waiting for all total permits.",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "pub fn shutdown(&self) {"
    },
    gap: Some(
        "Tests exercise router/library shutdown, not actual executable Ctrl-C, configuration validation or its ten-second drain deadline; wait_closed deadline exhaustion is not separately asserted."
    ),
});
