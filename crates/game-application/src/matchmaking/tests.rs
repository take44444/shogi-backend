use super::*;
fn ticket(n: u128, rating: i64, time: i64) -> Ticket {
    Ticket {
        user: UserId::new(format!("u{n}")).unwrap(),
        id: TicketId(Uuid::from_u128(n)),
        pool: Some(Pool::TenMinute30),
        state: TicketState::Waiting(Waiting {
            pool: Pool::TenMinute30,
            rating: RatingState::from_parts(Some(rating), 1).unwrap(),
            enqueued_at: time,
            expires_at: 1_000_000,
            heartbeat_seq: 0,
        }),
    }
}
#[test]
fn widening_is_bilateral_and_includes_boundary() {
    assert_eq!(window(0, 29_999), 100);
    assert_eq!(window(0, 30_000), 150);
    assert_eq!(window(i64::MIN, i64::MAX), 400);
    assert!(select_pair(&[ticket(1, 1500, 0), ticket(2, 1650, 30_000)], 30_000).is_none());
    assert!(select_pair(&[ticket(1, 1500, 0), ticket(2, 1650, 0)], 30_000).is_some());
    assert_eq!(RatingState::Unrated.effective(), 1500);
}

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
