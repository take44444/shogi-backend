use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Search は期間、取得件数、手数の境界を検証する。カーソルはページの取得件数ではなく、フィルターのフィンガープリントに結び付けて復号する。",
    source: trace::Source {
        path: "crates/game-record-application/src/lib.rs",
        needle: "pub fn new(filters: Filters, limit: u32, cursor: Option<&str>) -> Result<Self, Error> {"
    },
    gap: Some(
        "対応付けた事例は、フィンガープリントの全フィールドの変更や、カーソルのバイト数と UTF-8 の全境界を網羅しない。"
    ),
});
