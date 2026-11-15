crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Offline player OpenAPI JSON, no HTTP/database.",
        reason: "Package name/version are checked, with absent dependency-derived contact/license.",
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
