crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Offline user route/schema/local-reference assertions.",
        reason: "Exact paths/methods, cookie alternatives, headers, redirects, DTO fields, error enums and all local references are asserted.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn document_has_all_routes_real_schemas_and_cookie_security_alternatives() {
    let d = document();
    assert_eq!(
        d["paths"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "/auth/google/callback",
            "/auth/google/start",
            "/auth/logout",
            "/me",
            "/openapi.json"
        ]
    );
    for (path, methods) in [
        ("/auth/google/callback", vec!["get"]),
        ("/auth/google/start", vec!["get"]),
        ("/auth/logout", vec!["post"]),
        ("/me", vec!["get", "patch"]),
        ("/openapi.json", vec!["get"]),
    ] {
        assert_eq!(
            d["paths"][path]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            methods
        );
    }
    let schemes = &d["components"]["securitySchemes"];
    for (scheme, name) in [
        ("session", "__Host-user_session"),
        ("development_session", "user_session"),
        ("login", "__Host-user_login"),
        ("development_login", "user_login"),
    ] {
        assert_eq!(schemes[scheme]["type"], "apiKey");
        assert_eq!(schemes[scheme]["in"], "cookie");
        assert_eq!(schemes[scheme]["name"], name);
    }
    assert_eq!(
        d["paths"]["/auth/google/callback"]["get"]["security"],
        json!([{"login":[]},{"development_login":[]}])
    );
    for method in ["get", "patch"] {
        assert_eq!(
            d["paths"]["/me"][method]["security"],
            json!([{"session":[]},{"development_session":[]}])
        );
    }
    for (path, method) in [
        ("/auth/google/start", "get"),
        ("/auth/logout", "post"),
        ("/openapi.json", "get"),
    ] {
        assert!(d["paths"][path][method].get("security").is_none());
    }
    let params = d["paths"]["/auth/google/callback"]["get"]["parameters"]
        .as_array()
        .unwrap();
    assert_eq!(params.len(), 3);
    assert_eq!(
        params.iter().find(|p| p["name"] == "state").unwrap()["required"],
        true
    );
    for (path, method) in [("/me", "patch"), ("/auth/logout", "post")] {
        let params = d["paths"][path][method]["parameters"].as_array().unwrap();
        let origin = params.iter().find(|p| p["name"] == "Origin").unwrap();
        assert_eq!(origin["required"], true);
        assert_eq!(origin["in"], "header");
        let fetch = params
            .iter()
            .find(|p| p["name"] == "Sec-Fetch-Site")
            .unwrap();
        assert_eq!(fetch["required"], false);
        assert!(
            fetch["description"]
                .as_str()
                .unwrap()
                .contains("same-origin")
        );
    }
    for path in ["/auth/google/start", "/auth/google/callback"] {
        let redirect = &d["paths"][path]["get"]["responses"]["303"];
        assert!(redirect.get("content").is_none());
        assert!(redirect["headers"]["Location"].is_object());
        assert!(redirect["headers"]["Set-Cookie"].is_object());
    }
    assert!(
        d["paths"]["/auth/logout"]["post"]["responses"]["204"]
            .get("content")
            .is_none()
    );
    let schemas = &d["components"]["schemas"];
    assert_eq!(
        schemas["ProfileBody"]["required"],
        json!(["id", "display_name", "created_at"])
    );
    assert!(
        schemas["ProfileBody"]["properties"]["created_at"]["description"]
            .as_str()
            .unwrap()
            .contains("seconds")
    );
    assert_eq!(schemas["Rename"]["required"], json!(["display_name"]));
    assert_eq!(schemas["Rename"]["additionalProperties"], false);
    assert!(
        schemas["Rename"]["properties"]["display_name"]
            .get("maxLength")
            .is_none()
    );
    assert_eq!(
        schemas["ErrorCode"]["enum"],
        json!([
            "authentication_required",
            "invalid_input",
            "service_unavailable"
        ])
    );
    assert_local_refs_resolve(&d, &d);
}
}
