crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "テスト RSA で署名した claims を verify_response に渡す。実プロバイダーと DB は使わない。",
        reason: "誤った issuer・audience・nonce、nonce 欠落、期限切れ、誤った azp、複数 audience かつ azp 欠落を拒否し、一致する azp は受け入れる。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
}
