use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "PostgreSQL の commit_change は予想バージョンを比較し、メタデータ・受理記録・渡された固定イベント・終局に付随する記録を原子的に保存する。部分的な成功は公開しない。",
    source: trace::Source {
        path: "crates/game-persistence/src/lib.rs",
        needle: "async fn commit_change(&self, change: &StagedGameChange) -> Result<(), Error> {"
    },
    gap: None,
});
