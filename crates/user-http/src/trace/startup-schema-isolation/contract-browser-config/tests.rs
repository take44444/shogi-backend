use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Pure BrowserConfig validation; no environment mutation, Google request or process startup.",
        reason: "Direct cases accept HTTPS and explicit localhost/IPv6 loopback development, reject unsafe origins and compare the HTTPS callback URL.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn configuration_fails_closed() {
    for bad in [
        "http://users.example.com",
        "http://localhost:8080",
        "https://u:p@example.com",
        "https://example.com/path",
        "https://example.com?x=y",
        "https://example.com/#secret",
        "file:///tmp/file",
    ] {
        assert!(BrowserConfig::new(bad, false).is_err(), "{bad}");
    }
    for bad in [
        "http://evil.invalid",
        "http://localhost.evil.invalid",
        "http://192.168.1.2",
    ] {
        assert!(BrowserConfig::new(bad, true).is_err(), "{bad}");
    }
    assert!(BrowserConfig::new("http://localhost:8080", true).is_ok());
    assert!(BrowserConfig::new("http://[::1]:8080", true).is_ok());
    assert_eq!(
        BrowserConfig::new("https://users.example.com", false)
            .unwrap()
            .callback_url(),
        "https://users.example.com/auth/google/callback"
    );
}
}
