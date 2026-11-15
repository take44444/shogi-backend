use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Account startup loads ServerConfig, connects its store, discovers Google and then binds. Compose wires separate account storage, a distinct reader and a gateway without database credentials.",
    source: trace::Source {
        path: "apps/user-server/src/main.rs",
        needle: "async fn main() -> Result<(), Box<dyn std::error::Error>> {"
    },
    gap: Some(
        "No mapped Rust test launches every service or asserts ServerConfig::from_env, actual Google discovery availability, Ctrl-C/close or application readiness. Existing python3 scripts/test-deployment.py checks Compose topology, loopback exposure, origin routing, reader permissions and denied account access with Docker/PostgreSQL; this non-Cargo command remains outside traceability run selectors."
    ),
});
