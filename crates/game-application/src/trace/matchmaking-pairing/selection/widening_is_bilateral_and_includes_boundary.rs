crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Pure policy; no database or timer.",
        reason: "Requires both participants to have widened enough.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "matchmaking-pairing", contract: "windows", scope: "Pure policy; no database or timer.", relationship: crate::trace_support::Relationship::Asserts, reason: "Checks 29,999/30,000ms boundary, 400 cap, bilateral compatibility and Unrated=1500." }],
    };
#[test]
    fn widening_is_bilateral_and_includes_boundary() {
        assert_eq!(window(0, 29_999), 100);
        assert_eq!(window(0, 30_000), 150);
        assert_eq!(window(i64::MIN, i64::MAX), 400);
        assert!(select_pair(&[ticket(1, 1500, 0), ticket(2, 1650, 30_000)], 30_000).is_none());
        assert!(select_pair(&[ticket(1, 1500, 0), ticket(2, 1650, 0)], 30_000).is_some());
        assert_eq!(RatingState::Unrated.effective(), 1500);
    }
}
