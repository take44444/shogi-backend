use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "snapshot_jsonは閲覧者用スナップショットを、文字列の連番、所有権を持つ盤面・持ち駒、手番・王手・結果・合法手、固定された参加者の割り当てに変換する。上流の不正な列挙値・連番は拒否する。",
    source: trace::Source {
        path: "crates/realtime-gateway/src/game.rs",
        needle: "pub(crate) fn snapshot_json(snapshot: &proto::Snapshot) -> Result<Value, ()> {"
    },
    gap: Some(
        "選択したWebSocketの検証では、盤面・持ち駒の全フィールドや、上流の不正な値を変換する全分岐を比較しない。スナップショットを構成する値全体はgame-snapshots-accessで扱う。"
    ),
});
