use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "スナップショットの変換には、参加者の割り当て、シーケンス、駒がある升目、両者のすべての持ち駒、手番・王手・結果、合法 USI 手を含める。",
    source: trace::Source {
        path: "crates/game-grpc/src/conversion.rs",
        needle: "impl From<Snapshot> for proto::Snapshot {"
    },
    gap: Some(
        "選択した通信テストは、盤面と持ち駒の完全な順序や、すべての駒・列挙型の変換分岐を比較しない。"
    ),
});
