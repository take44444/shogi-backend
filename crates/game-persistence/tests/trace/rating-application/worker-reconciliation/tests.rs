use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の RatingWorker と PostgreSQL を使い、実際の適用コミットの前後で利用不能の結果を注入する。",
        reason: "実際のコミットの前後で利用不能の結果を注入すると、Applied または AlreadyApplied、続いて Idle となり、台帳は一件となる。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn worker_reconciles_errors_before_and_after_a_real_application_commit() {
    for after_commit in [false, true] {
        let f = Fixture::new().await;
        f.create("uncertain", Pool::TenMinute30).await;
        f.finish("uncertain").await;
        let repository = Arc::new(Uncertain {
            inner: f.jobs.clone(),
            first: AtomicBool::new(true),
            after_commit,
        });
        let worker = RatingWorker::new(repository, policy(30000, 3));
        assert_eq!(
            worker.tick(&owner("uncertain")).await.unwrap(),
            if after_commit {
                TickOutcome::AlreadyApplied
            } else {
                TickOutcome::Applied
            }
        );
        assert_eq!(
            worker.tick(&owner("uncertain")).await.unwrap(),
            TickOutcome::Idle
        );
        assert_eq!(f.count("rating_applications").await, 1);
        assert_eq!(f.count("rating_slots").await, 0);
    }
}
}
