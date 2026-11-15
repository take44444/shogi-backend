#[tokio::test]
async fn participation_is_global_and_refused_retries_remain_refused() {
    let f = Fixture::new().await;
    let first = TicketId::new();
    f.store
        .join(user("black"), first, Pool::TenMinute30)
        .await
        .unwrap();
    let second = TicketId::new();
    let refused = f
        .store
        .join(user("black"), second, Pool::Byoyomi30)
        .await
        .unwrap();
    assert_eq!(
        refused.ticket.state,
        TicketState::Refused(RefusalReason::AlreadyWaiting)
    );
    f.store.cancel(&user("black"), first).await.unwrap();
    assert_eq!(
        f.store
            .join(user("black"), second, Pool::Byoyomi30)
            .await
            .unwrap(),
        refused
    );
}
