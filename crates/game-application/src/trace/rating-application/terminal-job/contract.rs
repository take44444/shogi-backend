use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "対局のコミットは、レーティング対象対局の終局時にだけジョブを原子的に追加し、受理記録の再試行でもジョブを一件に保つ。",
    source: trace::Source {
        path: "crates/game-persistence/src/lib.rs",
        needle: "async fn commit_change(&self, change: &StagedGameChange) -> Result<(), Error> {"
    },
    gap: None,
});
