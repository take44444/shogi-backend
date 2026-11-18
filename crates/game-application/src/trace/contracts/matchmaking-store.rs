use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "matchmaking-store",
    consumer: "game-grpc",
    provider: "game-persistence",
    interface: trace::Interface::Port,
    statement: "所有者の参加・現在状態・継続・取消・準備確認を永続化する。参加の重複を防ぎ、期限内の双方の準備確認で対局と予約枠を原子的に作成する。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/game-application/src/matchmaking.rs",
        needle: "pub trait MatchmakingRepository: Send + Sync {"
    },
    call: Some(trace::Source {
        path: "crates/game-grpc/src/service.rs",
        needle: "async fn join_matchmaking("
    }),
    implementation: Some(trace::Source {
        path: "crates/game-persistence/src/matchmaking.rs",
        needle: "async fn join(&self, user: UserId, id: TicketId, pool: Pool) -> Result<JoinOutcome, MError> {"
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
