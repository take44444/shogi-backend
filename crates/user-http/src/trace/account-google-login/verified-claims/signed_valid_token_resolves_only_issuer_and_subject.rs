#[test]
fn signed_valid_token_resolves_only_issuer_and_subject() {
    let identity = verify(claims()).unwrap();
    assert_eq!(identity.issuer, GOOGLE_ISSUER);
    assert_eq!(identity.subject, "stable-subject");
    assert_eq!(identity.initial_name.as_str(), "Player");
}
