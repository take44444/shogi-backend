crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "署名済みテスト応答の access token と署名を個別に改変して検証する。",
        reason: "access token の置換と ID token 署名の改変を Unauthorized とする。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn substituted_access_token_and_tampered_signature_are_rejected() {
    assert!(matches!(
        verify_response(
            &client(),
            "client-id",
            response(claims(), "substituted"),
            "expected-nonce".into()
        ),
        Err(Error::Unauthorized)
    ));
    let mut value = serde_json::to_value(response(claims(), "access-token")).unwrap();
    let encoded = value["id_token"].as_str().unwrap();
    let mut parts = encoded.split('.').map(str::to_owned).collect::<Vec<_>>();
    let original = parts[2].remove(0);
    parts[2].insert(0, if original == 'a' { 'b' } else { 'a' });
    value["id_token"] = serde_json::Value::String(parts.join("."));
    let tampered: CoreTokenResponse = serde_json::from_value(value).unwrap();
    assert!(matches!(
        verify_response(&client(), "client-id", tampered, "expected-nonce".into()),
        Err(Error::Unauthorized)
    ));
}
}
