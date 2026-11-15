crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Generated user document status/media assertions without HTTP, storage or provider.",
        reason: "The documented PATCH response status set, JSON ErrorBody references, framework text schemas and callback text rejection are compared directly.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn document_distinguishes_json_errors_and_framework_text_rejections() {
    let d = document();
    let rename = &d["paths"]["/me"]["patch"]["responses"];
    assert_eq!(
        rename
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["200", "400", "401", "413", "415", "422", "503"]
    );
    for status in ["400", "401", "503"] {
        assert_eq!(
            rename[status]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ErrorBody"
        );
    }
    for status in ["400", "413", "415", "422"] {
        assert_eq!(
            rename[status]["content"]["text/plain"]["schema"]["type"],
            "string"
        );
    }
    assert!(
        d["paths"]["/auth/google/callback"]["get"]["responses"]["400"]["content"]["text/plain"]
            .is_object()
    );
}
}
