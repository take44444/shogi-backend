use super::*;
crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "共有 UserId の保持とバイト長・空白境界を直接確認する。公開 HTTP に任意 ID を送るテストではない。",
        reason: "ASCII・日本語・128 バイトの保持と、空文字・ASCII/Unicode 空白・129 バイト・多バイト長超過の拒否を直接確認する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "user-ratings", contract: "user-id", scope: "UserId の有効値の保持と長さ・空白の境界を直接確認する。HTTP パスの復号は通らない。", relationship: crate::trace_support::Relationship::Asserts, reason: "有効な ASCII・日本語・128 バイトの保持と、空文字・空白・129 バイト・多バイト長超過の拒否を確認する。" }],
    };
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
}
