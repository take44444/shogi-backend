use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "適用処理は固定された状態・現在状態・取得権を検証し、両者と適用台帳・ジョブ・予約枠を原子的に更新する。完了済み処理の再試行では、後の予約枠を変更せず照合する。",
    source: trace::Source {
        path: "crates/game-persistence/src/rating.rs",
        needle: "    async fn apply("
    },
    gap: None,
});
