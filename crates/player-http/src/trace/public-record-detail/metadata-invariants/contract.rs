use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "メタデータは、異なる参加者、終了時のシーケンス番号、保存層のスカラー値として有効な境界、および着手・投了に応じた終了時の手数と操作数の関係を要求する。",
    source: trace::Source {
        path: "crates/game-record-application/src/lib.rs",
        needle: "impl Metadata {"
    },
    gap: None,
});
