use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "ConnectionStateは連続して整合するコミット済みの内容だけを受け入れ、完全に同じ重複と古い境界イベントを破棄し、同じ連番の詰みの着手と終局の一致を確認する。",
    source: trace::Source {
        path: "crates/realtime-gateway/src/cursor.rs",
        needle: "pub(crate) fn accept(&mut self, notification: &Notification) -> Decision {"
    },
    gap: Some(
        "実際のWebSocketで詰みの組を検証するテストはない。カーソルの直接テストは人工的なコミット済み通知を使う。接続終了用の補助処理は、resync_requiredの後の切断と即時切断のどちらも許可する。"
    ),
});
