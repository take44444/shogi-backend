use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "rating-job-store",
    consumer: "game-application",
    provider: "game-persistence",
    interface: trace::Interface::Port,
    statement: "有効な取得権のもとで両者のレート・適用台帳・ジョブ完了・予約解放を原子的に適用する。再試行は適用済み結果を照合し、失効した取得権を拒否する。隔離解除は入力や予約を変更しない。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/game-application/src/rating.rs",
        needle: "pub trait RatingJobRepository: Send + Sync {"
    },
    call: Some(trace::Source {
        path: "crates/game-application/src/rating.rs",
        needle: "pub async fn tick(&self, owner: &WorkerId) -> Result<TickOutcome, RatingWorkerError> {"
    }),
    implementation: Some(trace::Source {
        path: "crates/game-persistence/src/rating.rs",
        needle: "impl RatingJobRepository for SeaOrmRatingJobs {"
    }),
    binding: Some(trace::Source {
        path: "apps/rating-worker-server/src/main.rs",
        needle: "let worker = RatingWorker::new(repository.clone(), policy);"
    }),
    requires: &[],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
