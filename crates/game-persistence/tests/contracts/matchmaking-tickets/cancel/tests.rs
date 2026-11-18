use super::*;

#[traceability::targets(
    game_persistence::SeaOrmStore::cancel,
    game_persistence::SeaOrmStore::tick
)]
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
        let other_ticket = active(&f.store, &uid(&other)).await;
        assert_eq!(other_ticket.id, other_id);
        match cancelled.state {
            TicketState::Cancelled => {
                assert!(matched.is_none());
                assert!(matches!(other_ticket.state, TicketState::Waiting(_)));
                f.store.cancel(&uid(&other), other_id).await.unwrap();
            }
            TicketState::Matched { game_id, .. } => {
                assert_eq!(matched, Some(game_id.clone()));
                assert!(
                    matches!(other_ticket.state,TicketState::Matched {game_id:other_game,..} if other_game==game_id)
                );
                assert_eq!(f.count("games").await, 0);
            }
            unexpected => panic!("unexpected race result: {unexpected:?}"),
        }
    }
}
