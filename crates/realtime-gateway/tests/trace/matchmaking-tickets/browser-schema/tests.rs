use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Inspects generated OpenAPI only; no service fixture or network calls.",
        reason: "All five method/path operations and both cookie schemes with alternative requirements are inspected.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "public-api-documents", contract: "gateway-schema", scope: "Pure gateway offline OpenAPI assertions; the integration target does not start services in this synchronous case.", relationship: crate::trace_support::Relationship::Asserts, reason: "The generated document contains GET /matchmaking/me, GET/PUT/DELETE ticket and POST heartbeat operations plus both cookie names and OR security alternatives." }],
    };
#[test]
fn matchmaking_openapi_covers_all_owner_scoped_routes() {
    let spec = serde_json::to_value(realtime_gateway::openapi()).unwrap();
    assert!(spec["paths"]["/matchmaking/me"]["get"].is_object());
    for method in ["get", "put", "delete"] {
        assert!(spec["paths"]["/matchmaking/tickets/{ticket_id}"][method].is_object());
    }
    assert!(spec["paths"]["/matchmaking/tickets/{ticket_id}/heartbeat"]["post"].is_object());
    assert_eq!(
        spec["components"]["securitySchemes"]["session"]["name"],
        "__Host-user_session"
    );
    assert_eq!(
        spec["components"]["securitySchemes"]["development_session"]["name"],
        "user_session"
    );
    assert_eq!(
        spec["security"],
        json!([{"session":[]},{"development_session":[]}])
    );
}
}
