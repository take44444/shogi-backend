use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "matchmaking-tick",
    consumer: "game-server",
    provider: "game-persistence",
    interface: trace::Interface::Port,
    statement: "期限内の待機チケットからレート差と決定的順序に従って組合せを確定し、失効した準備待ちを無効にする。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/game-application/src/matchmaking.rs",
        needle: "pub trait MatchmakingRepository: Send + Sync {"
    },
    call: Some(trace::Source {
        path: "apps/game-server/src/main.rs",
        needle: "match store.tick(pool).await {"
    }),
    implementation: Some(trace::Source {
        path: "crates/game-persistence/src/matchmaking.rs",
        needle: "async fn tick(&self, pool: Pool) -> Result<Option<GameId>, MError> {"
    }),
    binding: Some(trace::Source {
        path: "apps/game-server/src/main.rs",
        needle: "let matcher = tokio::spawn(async move {"
    }),
    requires: &[],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
