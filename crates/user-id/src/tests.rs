use super::*;
#[test]
fn preserves_valid_identifiers_and_rejects_byte_and_whitespace_boundaries() {
    for raw in ["user-123", "ユーザー", &"a".repeat(128)] {
        assert_eq!(UserId::new(raw).unwrap().as_str(), raw);
    }
    for raw in [
        "",
        "white space",
        "\tuser",
        "user\n",
        "user\u{3000}name",
        &"a".repeat(129),
        &"界".repeat(43),
    ] {
        assert_eq!(UserId::new(raw), Err(InvalidId));
    }
}
