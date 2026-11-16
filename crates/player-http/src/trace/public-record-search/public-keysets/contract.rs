use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "PostgreSQL の読み取り処理は、公開された終了済み対局のメタデータを対象に指定された全フィルターを適用し、直前の終了時刻とバイナリー形式の ID の組より後の結果だけを取得する。",
    source: trace::Source {
        path: "crates/game-record-persistence/src/lib.rs",
        needle: "async fn search(&self, search: &Search) -> Result<Page, Error> {"
    },
    gap: None,
});
