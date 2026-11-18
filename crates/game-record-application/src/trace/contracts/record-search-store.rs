use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "record-search-store",
    consumer: "game-record-application",
    provider: "game-record-persistence",
    interface: trace::Interface::Port,
    statement: "公開された終局済み対局だけを指定フィルターとキーセット順序で読み取り、書き込み操作を提供しない。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/game-record-application/src/lib.rs",
        needle: "pub trait RecordRepository: Send + Sync + 'static {"
    },
    call: Some(trace::Source {
        path: "crates/game-record-application/src/lib.rs",
        needle: "self.repository.search(search).await"
    }),
    implementation: Some(trace::Source {
        path: "crates/game-record-persistence/src/lib.rs",
        needle: "async fn search(&self, search: &Search) -> Result<Page, Error> {"
    }),
    binding: Some(trace::Source {
        path: "apps/player-server/src/main.rs",
        needle: "let repository = Arc::new(PostgresGameRecords::connect(&url).await?);"
    }),
    requires: &[trace::Dependency {
        contract: "game-record-persistence/postgres-search",
        reason: "提供元はこの直接の依存先を利用して、上位の応答と状態の保証を満たす。"
    }],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
