use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "期限内の Waiting チケットに対する、より大きいシーケンスだけが、データベース時刻を基準に有効期限を更新する。すべての終端状態は保存済み状態を保持する。",
    source: trace::Source {
        path: "crates/game-persistence/src/matchmaking.rs",
        needle: "async fn heartbeat("
    },
    gap: Some(
        "古いシーケンス、Waiting 以外の全状態、厳密な実時間の境界をすべて列挙する検証は、選択した根拠にはない。"
    ),
});
