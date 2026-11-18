use super::*;

#[test]
fn signed_valid_token_resolves_only_issuer_and_subject() {
    let identity = verify(claims()).unwrap();
    assert_eq!(identity.issuer, GOOGLE_ISSUER);
    assert_eq!(identity.subject, "stable-subject");
    assert_eq!(identity.initial_name.as_str(), "Player");
}

#[test]
fn signed_wrong_issuer_audience_nonce_expiry_or_azp_is_rejected() {
    let invalid = [
        claims().set_issuer(IssuerUrl::new("https://attacker.invalid".into()).unwrap()),
        claims().set_audiences(vec![Audience::new("another-client".into())]),
        claims().set_nonce(Some(Nonce::new("another-nonce".into()))),
        claims().set_nonce(None),
        claims().set_expiration(Utc::now() - Duration::seconds(1)),
        claims().set_authorized_party(Some(ClientId::new("another-client".into()))),
        claims().set_audiences(vec![
            Audience::new("client-id".into()),
            Audience::new("untrusted-client".into()),
        ]),
    ];
    for c in invalid {
        assert!(matches!(verify(c), Err(Error::Unauthorized)));
    }
    assert!(verify(claims().set_authorized_party(Some(ClientId::new("client-id".into())))).is_ok());
}

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
