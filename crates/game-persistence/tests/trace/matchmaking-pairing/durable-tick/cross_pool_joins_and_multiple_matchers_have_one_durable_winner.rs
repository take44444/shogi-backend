crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport.",
        reason: "Concurrent ticks produce exactly one game, one rated_game and two slots, and one user's current participation is Playing. This test does not compare participant or side bindings.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "admission", scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport. Concurrent admission and matcher tasks.", relationship: crate::trace_support::Relationship::Asserts, reason: "Concurrent different-pool joins produce exactly one created admission." }],
    };
#[tokio::test]
async fn cross_pool_joins_and_multiple_matchers_have_one_durable_winner() {
    let f = Fixture::new().await;
    let (a, b) = tokio::join!(
        f.store.join(uid("one"), TicketId::new(), POOL),
        f.store.join(uid("one"), TicketId::new(), Pool::Byoyomi30)
    );
    let results = [a.unwrap(), b.unwrap()];
    assert_eq!(results.iter().filter(|r| r.created).count(), 1);
    let pool = results
        .iter()
        .find(|r| r.created)
        .unwrap()
        .ticket
        .pool
        .unwrap();
    f.join("two", pool).await;
    let (a, b) = tokio::join!(f.store.tick(pool), f.store.tick(pool));
    assert_eq!(
        [a.unwrap(), b.unwrap()]
            .iter()
            .filter(|r| r.is_some())
            .count(),
        1
    );
    assert_eq!(f.count("games").await, 1);
    assert_eq!(f.count("rated_games").await, 1);
    assert_eq!(f.count("rating_slots").await, 2);
    assert!(
        matches!(f.store.current(&uid("one")).await.unwrap(),Participation::Active(t) if matches!(t.state,TicketState::Playing {..}))
    );
    let refusal = f
        .store
        .join(
            uid("one"),
            TicketId::new(),
            if pool == POOL { Pool::Byoyomi30 } else { POOL },
        )
        .await
        .unwrap();
    assert_eq!(
        refusal.ticket.state,
        TicketState::Refused(RefusalReason::AlreadyPlaying)
    );
}
}
