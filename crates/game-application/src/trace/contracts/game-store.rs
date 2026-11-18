use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "game-store",
    consumer: "game-application",
    provider: "game-persistence",
    interface: trace::Interface::Port,
    statement: "操作の受理記録・確定イベント・局面を原子的に保存し、再試行と競合時には永続結果を照合できる。閲覧用の読み取りはアクセス情報と履歴の整合性を保つ。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/game-application/src/repository.rs",
        needle: "pub trait GameRepository: Send + Sync {"
    },
    call: Some(trace::Source {
        path: "crates/game-application/src/actor.rs",
        needle: "async fn apply(&mut self, pending: MutationRequest) -> Result<Acceptance, ApplicationError> {"
    }),
    implementation: Some(trace::Source {
        path: "crates/game-persistence/src/lib.rs",
        needle: "async fn commit_change(&self, change: &StagedGameChange) -> Result<(), Error> {"
    }),
    binding: Some(trace::Source {
        path: "apps/game-server/src/main.rs",
        needle: "let service = GameRpc::new(GameService::new(store.clone()), store.clone());"
    }),
    requires: &[],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
