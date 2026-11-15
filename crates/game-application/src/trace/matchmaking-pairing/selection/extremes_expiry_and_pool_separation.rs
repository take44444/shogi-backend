crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Pure selector boundary and isolation fixtures.",
        reason: "Checks exclusive expiry boundary, extreme arithmetic and pool separation.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
    fn extremes_expiry_and_pool_separation() {
        assert!(select_pair(&[ticket(1, i64::MIN, 0), ticket(2, i64::MAX, 0)], 180_000).is_none());
        assert!(select_pair(&[ticket(1, 1500, 0), ticket(2, 1500, 0)], 1_000_000).is_none());
        let mut other = ticket(2, 1500, 0);
        if let TicketState::Waiting(state) = &mut other.state {
            state.pool = Pool::Byoyomi30;
        }
        assert!(select_pair(&[ticket(1, 1500, 0), other], 0).is_none());
        assert!(HeartbeatSequence::new(0).is_none());
        assert!(HeartbeatSequence::new(-1).is_none());
        assert_eq!(HeartbeatSequence::new(i64::MAX).unwrap().get(), i64::MAX);
    }
}
