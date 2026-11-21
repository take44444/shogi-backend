use super::*;

#[test]
fn matchmaking_openapi_covers_all_owner_scoped_routes() {
    let spec = serde_json::to_value(realtime_gateway::openapi()).unwrap();
    assert!(spec["paths"]["/matchmaking/me"]["get"].is_object());
    for method in ["put", "delete"] {
        assert!(spec["paths"]["/matchmaking/tickets/{ticket_id}"][method].is_object());
    }
    assert!(spec["paths"]["/matchmaking/tickets/{ticket_id}/heartbeat"]["post"].is_object());
    assert!(
        spec["paths"]["/matchmaking/tickets/{ticket_id}"]
            .get("get")
            .is_none()
    );
    assert_eq!(
        spec["paths"]
            .as_object()
            .unwrap()
            .values()
            .map(|path| path.as_object().unwrap().len())
            .sum::<usize>(),
        5
    );
    let ready = &spec["paths"]["/matchmaking/tickets/{ticket_id}/ready"]["post"];
    assert!(ready.is_object());
    assert!(ready.get("requestBody").is_none());
    for field in ["ready", "ready_deadline"] {
        assert!(spec["components"]["schemas"]["Ticket"]["properties"][field].is_object());
    }
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
