use super::*;
crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "DisplayName の値と境界を直接確認する。HTTP と保存層は通らない。",
        reason: "日本語名の trim、80 文字の受け入れ、空文字・空白・81 文字・改行・NUL の拒否を直接確認する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
}
