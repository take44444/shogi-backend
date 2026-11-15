crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Pure selector fixtures; no database or scheduler.",
        reason: "Checks skipped incompatible head and nearest/oldest/UUID preferences.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
    fn skips_unmatched_head_and_prefers_nearest_then_oldest_then_id() {
        let candidates = [
            ticket(1, 3000, 0),
            ticket(4, 1520, 3),
            ticket(3, 1480, 2),
            ticket(2, 1500, 1),
        ];
        let (a, b) = select_pair(&candidates, 3).unwrap();
        assert_eq!((a.id, b.id), (candidates[3].id, candidates[2].id));
        let candidates = [ticket(3, 1520, 0), ticket(2, 1500, 0), ticket(1, 1480, 0)];
        let (a, b) = select_pair(&candidates, 0).unwrap();
        assert_eq!((a.id, b.id), (candidates[2].id, candidates[1].id));
    }
}
