use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "rating-worker-tick",
    consumer: "rating-worker-server",
    provider: "game-application",
    interface: trace::Interface::RustApi,
    statement: "永続ジョブを一件取得して計算・適用し、成否不明時は適用結果を照合する。隔離ジョブは明示的な復旧要求でのみ再取得可能にする。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/game-application/src/rating.rs",
        needle: "pub async fn tick(&self, owner: &WorkerId) -> Result<TickOutcome, RatingWorkerError> {"
    },
    call: Some(trace::Source {
        path: "apps/rating-worker-server/src/main.rs",
        needle: "result=worker.tick(&owner)=> match result {"
    }),
    implementation: Some(trace::Source {
        path: "crates/game-application/src/rating.rs",
        needle: "pub async fn tick(&self, owner: &WorkerId) -> Result<TickOutcome, RatingWorkerError> {"
    }),
    binding: None,
    requires: &[trace::Dependency {
        contract: "game-application/rating-job-store",
        reason: "提供元はこの直接の依存先を利用して、上位の応答と状態の保証を満たす。"
    }],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
