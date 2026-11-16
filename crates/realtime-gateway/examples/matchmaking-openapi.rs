#[path = "../../../scripts/traceability_model.rs"]
mod trace_support;

trace_support::example!(trace_support::Example {
    kind: trace_support::ExampleKind::OpenApi,
    reason: "マッチメイキング用HTTP APIのOpenAPI文書をJSONとして出力する。",
});

fn main() {
    println!("{}", realtime_gateway::openapi().to_pretty_json().unwrap());
}
