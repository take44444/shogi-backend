use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。取消と組合せ処理を八回並行させて競合を検証する。",
        reason: "各競合では、対局を作らず取消をコミットするか、一つの予定対局に対応する二枚の Matched チケットをコミットし、対局はまだ作らない。",
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
        let other_ticket = active(&f.store, &uid(&other)).await;
        assert_eq!(other_ticket.id, other_id);
        match cancelled.state {
            TicketState::Cancelled => {
                assert!(matched.is_none());
                assert!(matches!(
                    other_ticket.state,
                    TicketState::Waiting(_)
                ));
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
}
