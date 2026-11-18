use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "rating-store",
    consumer: "game-record-application",
    provider: "game-record-persistence",
    interface: trace::Interface::Port,
    statement: "二つのプールの確定済みレーティングを一貫したスナップショットで返し、保留中の更新完了を約束しない。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/game-record-application/src/ratings.rs",
        needle: "pub trait RatingRepository: Send + Sync {"
    },
    call: Some(trace::Source {
        path: "crates/game-record-application/src/ratings.rs",
        needle: "self.repository.ratings(user).await"
    }),
    implementation: Some(trace::Source {
        path: "crates/game-record-persistence/src/ratings.rs",
        needle: "impl RatingRepository for PostgresGameRecords {"
    }),
    binding: Some(trace::Source {
        path: "apps/player-server/src/main.rs",
        needle: "let repository = Arc::new(PostgresGameRecords::connect(&url).await?);"
    }),
    requires: &[],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
