use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "DetailDto は内部のリクエスト、受領記録、王手、Outbox のフィールドを含めず、メタデータと順序付きの操作 DTO を出力する。",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "impl From<Detail> for DetailDto {"
    },
    gap: Some(
        "この検証は一つの着手と一つの投了を対象とし、すべての着手・駒打ち・終了結果のエンコードを網羅しない。"
    ),
});
