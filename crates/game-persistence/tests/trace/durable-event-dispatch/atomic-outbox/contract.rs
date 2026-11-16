use crate::trace_support::{self as trace, Contract};
trace::contract!(Contract {
    statement: "リポジトリのコミット処理は、予想シーケンスで対局更新を保護し、受理記録と固定された Outbox 行を同一トランザクションで保存する。アプリケーションの受理記録再試行は、重複コミットを防ぐ。",
    source: trace::Source {
        path: "crates/game-persistence/src/lib.rs",
        needle: "async fn commit_change(&self, change: &StagedGameChange) -> Result<(), Error> {"
    },
    gap: None
});
