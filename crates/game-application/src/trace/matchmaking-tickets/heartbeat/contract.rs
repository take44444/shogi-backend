use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "ハートビートは所有者の指定チケットの現在状態を返す。期限内の Waiting に対する、より大きいシーケンスだけがデータベース時刻を基準に待機期限を更新する。準備確認や終端状態は変更せず、期限切れを復活させない。",
    source: trace::Source {
        path: "crates/game-persistence/src/matchmaking.rs",
        needle: "async fn heartbeat("
    },
    gap: Some(
        "古いシーケンス、Waiting 以外の全状態、厳密な実時間の境界をすべて列挙する検証は、選択した根拠にはない。"
    ),
});
