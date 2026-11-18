use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "postgres-search",
    consumer: "game-record-persistence",
    provider: "postgres",
    interface: trace::Interface::External,
    statement: "指定した公開状態・フィルター・終了時刻とバイナリー ID の順序に従う検索を実行し、読み取り接続で更新操作を行わない。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/game-record-persistence/src/lib.rs",
        needle: "async fn search(&self, search: &Search) -> Result<Page, Error> {"
    },
    call: Some(trace::Source {
        path: "crates/game-record-persistence/src/lib.rs",
        needle: "async fn search(&self, search: &Search) -> Result<Page, Error> {"
    }),
    implementation: None,
    binding: None,
    requires: &[],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
