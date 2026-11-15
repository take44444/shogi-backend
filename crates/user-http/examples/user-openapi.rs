#[path = "../../../scripts/traceability_model.rs"]
mod trace_support;

trace_support::example!(trace_support::Example {
    kind: trace_support::ExampleKind::OpenApi,
    reason: "Prints the account HTTP OpenAPI document as JSON.",
});

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_http::openapi().to_pretty_json()?);
    Ok(())
}
