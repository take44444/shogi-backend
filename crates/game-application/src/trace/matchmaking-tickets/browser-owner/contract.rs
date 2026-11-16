use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "ブラウザーハンドラーは型付きの参加処理に入る前に、セッション認証から所有者を導出し、Host の完全一致、変更時に必須の Origin、任意の same-site メタデータを確認する。",
    source: trace::Source {
        path: "crates/realtime-gateway/src/matchmaking.rs",
        needle: "async fn authenticate("
    },
    gap: Some(
        "すべての不正 UUID、DELETE 本文の拒否、HTTP 許可枠の飽和、認証や接続先のタイムアウト分岐を網羅する専用の検証はない。"
    ),
});
