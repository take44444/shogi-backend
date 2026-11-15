crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport. Eight concurrent cancellation/matcher races.",
        reason: "Each race commits either cancellation with no game or two Playing tickets for one game.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn cancellation_racing_a_match_has_one_committed_outcome() {
    let f = Fixture::new().await;
    for n in 0..8 {
        let user = format!("one-{n}");
        let other = format!("two-{n}");
        let id = f.join(&user, POOL).await;
        let other_id = f.join(&other, POOL).await;
        let owner = uid(&user);
        let (cancelled, matched) = tokio::join!(f.store.cancel(&owner, id), f.store.tick(POOL));
        let cancelled = cancelled.unwrap();
        let matched = matched.unwrap();
        match cancelled.state {
            TicketState::Cancelled => {
                assert!(matched.is_none());
                assert!(matches!(
                    f.store.ticket(&uid(&other), other_id).await.unwrap().state,
                    TicketState::Waiting(_)
                ));
                f.store.cancel(&uid(&other), other_id).await.unwrap();
            }
            TicketState::Playing { game_id, .. } => {
                assert_eq!(matched, Some(game_id.clone()));
                assert!(
                    matches!(f.store.ticket(&uid(&other),other_id).await.unwrap().state,TicketState::Playing {game_id:other_game,..} if other_game==game_id)
                );
            }
            unexpected => panic!("unexpected race result: {unexpected:?}"),
        }
    }
}
}
