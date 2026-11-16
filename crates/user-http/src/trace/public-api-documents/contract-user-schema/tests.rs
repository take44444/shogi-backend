use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "ユーザーサービスのオフライン OpenAPI JSON。外部認証プロバイダーと保存層は使わない。",
        reason: "パッケージ名とバージョンを確認し、依存先由来の連絡先やライセンスの表記がないことを確認する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn document_identifies_this_service_without_dependency_attribution() {
    let d = document();
    assert_eq!(d["info"]["title"], env!("CARGO_PKG_NAME"));
    assert_eq!(d["info"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(d["info"].get("contact").is_none());
    assert!(d["info"].get("license").is_none());
}
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "ユーザーサービスのルート、スキーマ、ローカル参照をオフラインで検証する。",
        reason: "パスと HTTP メソッドの完全一致、Cookie の選択肢、ヘッダー、リダイレクト、DTO のフィールド、エラーの列挙値、すべてのローカル参照を検証する。",
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

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "HTTP、保存層、外部認証プロバイダーを使わず、ユーザーサービスの生成文書にあるステータスとメディア型を検証する。",
        reason: "文書に記載された PATCH 応答のステータス集合、JSON の ErrorBody への参照、フレームワークのテキストスキーマ、コールバックのテキストによる入力拒否を直接比較する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn document_distinguishes_json_errors_and_framework_text_rejections() {
    let d = document();
    let rename = &d["paths"]["/me"]["patch"]["responses"];
    assert_eq!(
        rename
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["200", "400", "401", "413", "415", "422", "503"]
    );
    for status in ["400", "401", "503"] {
        assert_eq!(
            rename[status]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ErrorBody"
        );
    }
    for status in ["400", "413", "415", "422"] {
        assert_eq!(
            rename[status]["content"]["text/plain"]["schema"]["type"],
            "string"
        );
    }
    assert!(
        d["paths"]["/auth/google/callback"]["get"]["responses"]["400"]["content"]["text/plain"]
            .is_object()
    );
}
}
