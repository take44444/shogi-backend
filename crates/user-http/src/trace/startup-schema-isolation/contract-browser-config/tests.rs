use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "BrowserConfig の値を直接検証する。環境変数の変更、Google へのリクエスト、プロセスの起動は行わない。",
        reason: "HTTPS と、明示的に指定した localhost・IPv6 ループバックでの開発用設定を受け入れること、安全でない Origin を拒否すること、HTTPS のコールバック URL が一致することを直接確認する。",
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
