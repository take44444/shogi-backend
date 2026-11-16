use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "公開対局サービスのオフライン OpenAPI JSON。HTTP とデータベースは使わない。",
        reason: "パッケージ名とバージョンを確認し、依存先由来の連絡先とライセンスがないことを確認する。",
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
        scope: "公開対局サービスのルートとスキーマをオフラインで検証する。サービスは起動しない。",
        reason: "OpenAPI 3.1、四つのパスと HTTP メソッドの完全一致、パラメーターの制約、列挙したエラー、ローカル参照を解決できることを検証する。",
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

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "公開対局サービスの OpenAPI と実際の DTO のシリアライズ。保存層は使わない。",
        reason: "平坦化した操作、必須かつ null を許すカーソル、符号なし整数の境界を、実際の DTO の JSON と比較する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn response_schemas_preserve_flattened_actions_required_null_and_unsigned_integers() {
    let d = document();
    let schemas = &d["components"]["schemas"];
    assert_eq!(
        schemas["PageDto"]["required"],
        json!(["items", "next_cursor"])
    );
    assert_eq!(
        schemas["PageDto"]["properties"]["next_cursor"]["type"],
        json!(["string", "null"])
    );
    assert_eq!(
        schemas["MetadataDto"]["required"],
        json!([
            "id",
            "black",
            "white",
            "rule",
            "result",
            "finished_at",
            "final_seq",
            "final_move_ply"
        ])
    );
    assert_eq!(
        schemas["DetailDto"]["required"],
        json!(["metadata", "history"])
    );
    assert_eq!(
        schemas["OperationDto"]["allOf"][0]["$ref"],
        "#/components/schemas/ActionDto"
    );
    assert_eq!(
        schemas["OperationDto"]["allOf"][1]["required"],
        json!(["seq", "player"])
    );
    let variants = schemas["ActionDto"]["oneOf"].as_array().unwrap();
    assert_eq!(variants.len(), 2);
    assert_eq!(variants[0]["required"], json!(["usi", "action"]));
    assert_eq!(variants[0]["properties"]["action"]["enum"], json!(["move"]));
    assert_eq!(variants[1]["required"], json!(["action"]));
    assert_eq!(
        variants[1]["properties"]["action"]["enum"],
        json!(["resign"])
    );
    for schema in [
        &schemas["MetadataDto"]["properties"]["final_seq"],
        &schemas["MetadataDto"]["properties"]["final_move_ply"],
        &schemas["OperationDto"]["allOf"][1]["properties"]["seq"],
    ] {
        assert_eq!(schema["minimum"], 0);
        assert_eq!(schema["maximum"], u64::MAX);
        assert_eq!(schema["format"], "uint64");
    }
    let movement = OperationDto {
        seq: u64::MAX,
        player: PlayerDto::Black,
        action: ActionDto::Move { usi: "7g7f".into() },
    };
    let resign = OperationDto {
        seq: 2,
        player: PlayerDto::White,
        action: ActionDto::Resign,
    };
    assert_eq!(
        serde_json::to_value(movement).unwrap(),
        json!({"seq":u64::MAX,"player":"black","action":"move","usi":"7g7f"})
    );
    assert_eq!(
        serde_json::to_value(resign).unwrap(),
        json!({"seq":2,"player":"white","action":"resign"})
    );
    assert_eq!(
        serde_json::to_value(PageDto {
            items: vec![],
            next_cursor: None
        })
        .unwrap(),
        json!({"items":[],"next_cursor":null})
    );
}
}
