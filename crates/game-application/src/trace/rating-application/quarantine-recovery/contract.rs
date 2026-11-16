use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "永続層の失敗処理と復旧処理は、予約枠を保持したまま一時的失敗後の待機時間と試行回数の枠を適用する。復旧は状態を修復せず隔離を解除する。",
    source: trace::Source {
        path: "crates/game-persistence/src/rating.rs",
        needle: "    async fn retry_quarantined("
    },
    gap: Some(
        "復旧テストは、結果、取得世代の増加、予約枠の保持、破損状態の修復が必要なことを検証する。復旧前後で、固定入力と両者のレーティングの完全なスナップショットを比較してはいない。"
    ),
});
