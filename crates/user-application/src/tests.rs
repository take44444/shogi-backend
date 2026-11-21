use super::*;
#[test]
fn display_names_are_trimmed_and_bound_in_unicode_characters() {
    assert_eq!(
        DisplayName::new("  将棋  ".into()).unwrap().as_str(),
        "将棋"
    );
    assert!(DisplayName::new("界".repeat(80)).is_ok());
    for name in [
        "".into(),
        "  ".into(),
        "界".repeat(81),
        "a\nb".into(),
        "a\0b".into(),
    ] {
        assert_eq!(DisplayName::new(name), Err(Error::InvalidInput));
    }
}
