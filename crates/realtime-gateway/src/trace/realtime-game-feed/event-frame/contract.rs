use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "event_jsonは、対局・連番・種別と着手・結果のフィールドを持つ安定したイベント識別情報を出力し、内部のユーザーIDとリクエストIDを含めない。",
    source: trace::Source {
        path: "crates/realtime-gateway/src/cursor.rs",
        needle: "pub(crate) fn event_json(notification: &Notification) -> Value {"
    },
    gap: Some(
        "event_idの16進エンコードや、すべての対局識別子に対する一意性を証明する直接の検証はない。"
    ),
});
