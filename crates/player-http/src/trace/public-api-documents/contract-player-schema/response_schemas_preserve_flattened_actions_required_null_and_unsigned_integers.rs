crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Player OpenAPI and actual DTO serialization, no storage.",
        reason: "Assertions compare flattened actions, required nullable cursor and unsigned integer bounds with actual DTO JSON.",
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
