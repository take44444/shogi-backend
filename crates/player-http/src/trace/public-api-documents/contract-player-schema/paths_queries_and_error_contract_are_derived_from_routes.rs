crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Offline player route and schema assertions, no service startup.",
        reason: "OpenAPI 3.1, exact four paths and methods, parameter constraints, enumerated errors and resolvable local refs are asserted.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn paths_queries_and_error_contract_are_derived_from_routes() {
    let d = document();
    assert_eq!(d["openapi"], "3.1.0");
    assert_eq!(
        d["paths"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "/game-records",
            "/game-records/{id}",
            "/openapi.json",
            "/users/{user_id}/ratings"
        ]
    );
    for path in [
        "/game-records",
        "/game-records/{id}",
        "/openapi.json",
        "/users/{user_id}/ratings",
    ] {
        assert_eq!(
            d["paths"][path]
                .as_object()
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            ["get"]
        );
    }
    let params = d["paths"]["/game-records"]["get"]["parameters"]
        .as_array()
        .unwrap();
    assert_eq!(params.len(), 10);
    let param = |name| params.iter().find(|p| p["name"] == name).unwrap();
    for name in ["from", "to"] {
        assert_eq!(param(name)["required"], true);
    }
    for name in [
        "participant",
        "rule",
        "winner",
        "reason",
        "min_ply",
        "max_ply",
        "limit",
        "cursor",
    ] {
        assert_eq!(param(name)["required"], false);
    }
    assert_eq!(param("from")["schema"]["minimum"], 0);
    assert_eq!(param("to")["schema"]["minimum"], 1);
    assert!(
        param("from")["description"]
            .as_str()
            .unwrap()
            .contains("366 days")
    );
    for name in ["min_ply", "max_ply"] {
        assert_eq!(param(name)["schema"]["minimum"], 0);
        assert_eq!(param(name)["schema"]["maximum"], i64::MAX);
    }
    assert_eq!(param("limit")["schema"]["minimum"], 1);
    assert_eq!(param("limit")["schema"]["maximum"], 100);
    assert_eq!(param("limit")["schema"]["default"], 30);
    assert!(param("cursor")["schema"].get("maxLength").is_none());
    assert!(param("participant")["schema"].get("maxLength").is_none());
    for (name, values) in [
        ("RuleDto", json!(["standard"])),
        ("PlayerDto", json!(["black", "white"])),
        ("ReasonDto", json!(["checkmate", "resignation"])),
    ] {
        assert_eq!(d["components"]["schemas"][name]["enum"], values);
    }
    for path in ["/game-records", "/game-records/{id}"] {
        let responses = &d["paths"][path]["get"]["responses"];
        for status in ["200", "400", "422", "500", "503"] {
            assert!(responses[status]["content"]["application/json"].is_object());
        }
        assert_eq!(responses.get("404").is_some(), path.ends_with("{id}"));
    }
    assert_eq!(
        d["components"]["schemas"]["ErrorCode"]["enum"],
        json!([
            "invalid_query",
            "not_found",
            "invalid_record",
            "unavailable",
            "history_too_long"
        ])
    );
    assert_local_refs_resolve(&d, &d);
}
}
