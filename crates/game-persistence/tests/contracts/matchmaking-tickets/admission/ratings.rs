use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/matchmaking-store", relationship: crate::trace_support::Relationship::Unreviewed, scope: "製品の公開 API を呼ぶ結合テスト。個々の assertion と契約の対応は未レビュー。", reason: "宣言した実提供元の境界を利用する。契約の各条件を直接検証するかは未レビュー。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-persistence"], fake: &[] }] };
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
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/matchmaking-store", relationship: crate::trace_support::Relationship::Unreviewed, scope: "製品の公開 API を呼ぶ結合テスト。個々の assertion と契約の対応は未レビュー。", reason: "宣言した実提供元の境界を利用する。契約の各条件を直接検証するかは未レビュー。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-persistence"], fake: &[] }] };
#[tokio::test]
async fn admission_waits_for_application_and_freezes_the_new_committed_states() {
    let f = Fixture::new().await;
    f.create("previous", Pool::TenMinute30).await;
    f.finish("previous").await;
    let claim = f
        .jobs
        .claim(&owner("applying"), &policy(30000, 3))
        .await
        .unwrap()
        .unwrap();
    let delta = change(&claim);
    let held = f.raw.begin().await.unwrap();
    held.execute_unprepared("SELECT * FROM rating_states FOR UPDATE")
        .await
        .unwrap();
    let jobs = f.jobs.clone();
    let applying = tokio::spawn(async move { jobs.apply(&claim, &delta).await });
    wait_for_blocked_transactions(&f, 1).await;
    let store = f.store.clone();
    let admission = tokio::spawn(async move {
        store
            .join(user("black"), TicketId::new(), Pool::TenMinute30)
            .await
    });
    wait_for_blocked_transactions(&f, 2).await;
    held.commit().await.unwrap();
    assert_eq!(applying.await.unwrap().unwrap(), ApplyOutcome::Applied);
    let admitted = admission.await.unwrap().unwrap();
    assert!(admitted.created);
    let white_ticket = TicketId::new();
    f.store
        .join(user("white"), white_ticket, Pool::TenMinute30)
        .await
        .unwrap();
    let next = f.store.tick(Pool::TenMinute30).await.unwrap().unwrap();
    f.store
        .ready(&user("black"), admitted.ticket.id)
        .await
        .unwrap();
    f.store.ready(&user("white"), white_ticket).await.unwrap();
    let identity = pg_test_support::rated_identity(f.database.url(), &next)
        .await
        .unwrap();
    let (black, white) = if identity.participants.black() == &user("black") {
        (identity.black_start, identity.white_start)
    } else {
        (identity.white_start, identity.black_start)
    };
    assert_eq!(black.rating(), Some(1516));
    assert_eq!(white.rating(), Some(1484));
    assert_eq!(black.completed(), 1);
    assert_eq!(white.completed(), 1);
    assert_eq!(f.count("rating_slots").await, 2);
}
}
