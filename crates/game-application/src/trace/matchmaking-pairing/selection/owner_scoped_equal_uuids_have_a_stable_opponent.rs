crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Pure selector with equal UUIDs and distinct owners.",
        reason: "Equal owner-scoped ticket UUIDs have deterministic owner tie breaking.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
    fn owner_scoped_equal_uuids_have_a_stable_opponent() {
        let anchor = ticket(1, 1500, 0);
        let first = ticket(2, 1520, 1);
        let mut second = first.clone();
        second.user = UserId::new("u3").unwrap();
        for tickets in [
            [anchor.clone(), first.clone(), second.clone()],
            [second.clone(), anchor.clone(), first.clone()],
        ] {
            assert_eq!(select_pair(&tickets, 1).unwrap().1.user, first.user);
        }
    }
}
