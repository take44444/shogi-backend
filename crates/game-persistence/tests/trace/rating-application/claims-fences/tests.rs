use super::*;

async fn recover_job(f: &Fixture, policy: &WorkerPolicy) -> ClaimedRatingJob {
    f.create("recovered", Pool::TenMinute30).await;
    f.finish("recovered").await;
    let claim = f.jobs.claim(&owner("old"), policy).await.unwrap().unwrap();
    assert_eq!(
        f.jobs
            .fail(&claim, JobFailure::Corrupt, policy)
            .await
            .unwrap(),
        TickOutcome::Quarantined
    );
    assert_eq!(
        f.jobs.retry_quarantined(&game("recovered")).await.unwrap(),
        RecoveryOutcome::Retried
    );
    claim
}

fn claim_on_independent_runtime(f: &Fixture, policy: WorkerPolicy) -> Option<GameId> {
    let url = f.database.url().to_owned();
    // Joining pauses this test's runtime so deferred rollback cannot release the job.
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                tokio::time::timeout(Duration::from_secs(10), async {
                    let jobs = SeaOrmRatingJobs::connect(&url).await.unwrap();
                    let acquired = jobs.claim(&owner("replacement"), &policy).await.unwrap();
                    jobs.close().await.unwrap();
                    acquired.map(|job| job.game_id().clone())
                })
                .await
                .expect("independent claim finished while cleanup runtime was paused")
            })
    })
    .join()
    .unwrap()
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の PostgreSQL で、復旧後の古い取得権への失敗通知と、返却直後の別 runtime による取得を検証する。",
        reason: "失敗通知が LostClaim を返した時点で、復旧したジョブを別のワーカーが取得できる。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "rating-application", contract: "quarantine-recovery", scope: "実際の PostgreSQL で、復旧後の古い取得権への失敗通知と、返却直後の別 runtime による取得を検証する。", relationship: crate::trace_support::Relationship::Asserts, reason: "隔離解除後のジョブは、古い取得権への失敗通知を拒否した直後にも取得できる。" }],
    };
#[tokio::test]
async fn stale_failure_releases_recovered_job_before_returning() {
    let f = Fixture::new().await;
    let policy = policy(30000, 3);
    let old = recover_job(&f, &policy).await;
    assert_eq!(
        f.jobs.fail(&old, JobFailure::Transient, &policy).await.unwrap(),
        TickOutcome::LostClaim
    );
    assert_eq!(claim_on_independent_runtime(&f, policy), Some(game("recovered")));
}
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の PostgreSQL で、復旧後の古い取得権での適用と、返却直後の別 runtime による取得を検証する。",
        reason: "適用が LostClaim を返した時点で、復旧したジョブを別のワーカーが取得できる。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "rating-application", contract: "quarantine-recovery", scope: "実際の PostgreSQL で、復旧後の古い取得権での適用と、返却直後の別 runtime による取得を検証する。", relationship: crate::trace_support::Relationship::Asserts, reason: "隔離解除後のジョブは、古い取得権での適用を拒否した直後にも取得できる。" }],
    };
#[tokio::test]
async fn stale_application_releases_recovered_job_before_returning() {
    let f = Fixture::new().await;
    let policy = policy(30000, 3);
    let old = recover_job(&f, &policy).await;
    assert_eq!(
        f.jobs.apply(&old, &change(&old)).await.unwrap(),
        ApplyOutcome::LostClaim
    );
    assert_eq!(claim_on_independent_runtime(&f, policy), Some(game("recovered")));
}
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の PostgreSQL で、隔離されていないジョブの復旧結果と、返却直後の別 runtime による取得を検証する。",
        reason: "復旧が NotQuarantined を返した時点で、実行可能なジョブを別のワーカーが取得できる。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "rating-application", contract: "quarantine-recovery", scope: "実際の PostgreSQL で、隔離されていないジョブの復旧結果と、返却直後の別 runtime による取得を検証する。", relationship: crate::trace_support::Relationship::Asserts, reason: "復旧を繰り返して NotQuarantined となっても、復旧済みのジョブを直ちに取得できる。" }],
    };
#[tokio::test]
async fn repeated_recovery_releases_recovered_job_before_returning() {
    let f = Fixture::new().await;
    let policy = policy(30000, 3);
    recover_job(&f, &policy).await;
    assert_eq!(
        f.jobs.retry_quarantined(&game("recovered")).await.unwrap(),
        RecoveryOutcome::NotQuarantined
    );
    assert_eq!(claim_on_independent_runtime(&f, policy), Some(game("recovered")));
}
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の PostgreSQL で、取得権、期限切れと古い取得権による操作の拒否、試行回数の枠、復旧を検証する。ワーカーのエラー処理経路は対象としない。",
        reason: "取得世代が増加し、期限切れまたは古い取得権での適用と失敗通知が LostClaim となること、取得回数の枠を使い切っても参加枠を保持すること、復旧結果を確認する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "rating-application", contract: "quarantine-recovery", scope: "実際の PostgreSQL で、取得権、期限切れと古い取得権による操作の拒否、試行回数の枠、復旧を検証する。ワーカーのエラー処理経路は対象としない。", relationship: crate::trace_support::Relationship::Asserts, reason: "失効した取得権の試行枠を使い切っても参加枠を保持する。復旧と、復旧の繰り返しや不明な対象に対する結果を直接確認する。" }],
    };
#[tokio::test]
async fn expired_claims_consume_budget_quarantine_and_recover_without_releasing_slots() {
    let f = Fixture::new().await;
    f.create("expiry", Pool::TenMinute30).await;
    f.finish("expiry").await;
    let policy = policy(30000, 2);
    let old = f.jobs.claim(&owner("old"), &policy).await.unwrap().unwrap();
    f.raw
        .execute_unprepared("UPDATE rating_jobs SET lease_until=0")
        .await
        .unwrap();
    assert_eq!(
        f.jobs
            .fail(&old, JobFailure::Transient, &policy)
            .await
            .unwrap(),
        TickOutcome::LostClaim
    );
    let new = f.jobs.claim(&owner("new"), &policy).await.unwrap().unwrap();
    assert!(new.generation() > old.generation());
    assert_eq!(
        f.jobs.apply(&old, &change(&old)).await.unwrap(),
        ApplyOutcome::LostClaim
    );
    assert_eq!(
        f.jobs
            .fail(&old, JobFailure::Corrupt, &policy)
            .await
            .unwrap(),
        TickOutcome::LostClaim
    );
    f.raw
        .execute_unprepared("UPDATE rating_jobs SET lease_until=0")
        .await
        .unwrap();
    assert!(
        f.jobs
            .claim(&owner("third"), &policy)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(f.count("rating_slots").await, 2);
    assert_eq!(f.count("rating_applications").await, 0);
    assert_eq!(
        f.jobs.retry_quarantined(&game("expiry")).await.unwrap(),
        RecoveryOutcome::Retried
    );
    assert_eq!(
        f.jobs.retry_quarantined(&game("expiry")).await.unwrap(),
        RecoveryOutcome::NotQuarantined
    );
    assert_eq!(
        f.jobs.retry_quarantined(&game("missing")).await.unwrap(),
        RecoveryOutcome::NotFound
    );
    assert_eq!(
        f.jobs
            .fail(&new, JobFailure::Transient, &policy)
            .await
            .unwrap(),
        TickOutcome::LostClaim
    );
    let recovered = f
        .jobs
        .claim(&owner("recovered"), &policy)
        .await
        .unwrap()
        .unwrap();
    assert!(recovered.generation() > new.generation());
    assert_eq!(
        f.jobs.apply(&recovered, &change(&recovered)).await.unwrap(),
        ApplyOutcome::Applied
    );
    assert_eq!(f.count("rating_slots").await, 0);
}
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の PostgreSQL で、ロック待ち中にリースが失効する場合を検証する。CLI は使わない。",
        reason: "状態のロック待ち中にリースが切れると LostClaim となり、台帳を作らず二つの予約枠を保持し、先手の未評価状態を変更しない。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn lease_expiring_while_waiting_for_state_lock_rolls_back_all_mutations() {
    let f = Fixture::new().await;
    f.create("wait", Pool::TenMinute30).await;
    f.finish("wait").await;
    let claim = f
        .jobs
        .claim(&owner("waiting"), &policy(300, 3))
        .await
        .unwrap()
        .unwrap();
    let delta = change(&claim);
    let held = f.raw.begin().await.unwrap();
    held.execute_unprepared("SELECT * FROM rating_states FOR UPDATE")
        .await
        .unwrap();
    let jobs = f.jobs.clone();
    let task = tokio::spawn(async move { jobs.apply(&claim, &delta).await });
    tokio::time::timeout(Duration::from_secs(3),async {
        loop {
            let n:i64=f.raw.query_one_raw(Statement::from_string(DbBackend::Postgres,"SELECT count(*) AS n FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock'")).await.unwrap().unwrap().try_get("","n").unwrap();
            if n>0 {break}tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.unwrap();
    f.raw
        .execute_unprepared("SELECT pg_sleep(0.35)")
        .await
        .unwrap();
    held.commit().await.unwrap();
    assert_eq!(task.await.unwrap().unwrap(), ApplyOutcome::LostClaim);
    assert_eq!(f.count("rating_applications").await, 0);
    assert_eq!(f.count("rating_slots").await, 2);
    assert_eq!(
        f.reader()
            .await
            .ratings(&user("black"))
            .await
            .unwrap()
            .ten_minute_30,
        RatingState::Unrated
    );
}
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の PostgreSQL で、行ロックを保持した状態と同時取得を検証する。",
        reason: "ロック中のジョブを読み飛ばし、残るジョブを同時に取得しようとした二つの処理のうち、一つだけが所有権を得る。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn concurrent_workers_acquire_disjoint_jobs_and_skip_a_locked_job() {
    let f = Fixture::new().await;
    f.create("locked", Pool::TenMinute30).await;
    f.finish("locked").await;
    f.create("available", Pool::Byoyomi30).await;
    f.finish("available").await;
    let held = f.raw.begin().await.unwrap();
    held.execute_unprepared("SELECT * FROM rating_jobs WHERE game_id='locked' FOR UPDATE")
        .await
        .unwrap();
    let acquired = tokio::time::timeout(
        Duration::from_secs(2),
        f.jobs.claim(&owner("worker"), &policy(30000, 3)),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    assert_eq!(acquired.game_id(), &game("available"));
    held.commit().await.unwrap();
    let first_owner = owner("one");
    let second_owner = owner("two");
    let claim_policy = policy(30000, 3);
    let (first, second) = tokio::join!(
        f.jobs.claim(&first_owner, &claim_policy),
        f.jobs.claim(&second_owner, &claim_policy)
    );
    let claims = [first.unwrap(), second.unwrap()];
    assert_eq!(claims.iter().filter(|c| c.is_some()).count(), 1);
    assert_eq!(
        claims.into_iter().flatten().next().unwrap().game_id(),
        &game("locked")
    );
}
}
