use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/rating-job-store", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の RatingWorker と PostgreSQL を使い、実際の適用コミットの前後で利用不能の結果を注入する。", reason: "実際のコミットの前後で利用不能の結果を注入すると、Applied または AlreadyApplied、続いて Idle となり、台帳は一件となる。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-persistence"], fake: &[] }] };
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
