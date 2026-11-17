use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "生成されたOpenAPIだけを検査する。サービスの準備処理やネットワーク呼び出しは使わない。",
        reason: "5件すべてのメソッド・パスと、選択肢として使える2種類のCookie認証方式を検査する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "public-api-documents", contract: "gateway-schema", scope: "ゲートウェイのオフラインOpenAPIだけを検証する。この同期テストでは結合テストのtargetからサービスを起動しない。", relationship: crate::trace_support::Relationship::Asserts, reason: "生成された文書に、GET /matchmaking/me、チケットのPUT・DELETE、ハートビート・準備完了のPOSTと、2種類のCookie名、およびいずれかで認証するOR条件が含まれることを確認する。" }],
    };
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
}
