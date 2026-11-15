use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "snapshot_json converts viewer snapshot to string sequence, owned board/hands, turn/check/result/legal moves and permanent participant bindings, rejecting invalid upstream enum/sequence values.",
    source: trace::Source {
        path: "crates/realtime-gateway/src/game.rs",
        needle: "pub(crate) fn snapshot_json(snapshot: &proto::Snapshot) -> Result<Value, ()> {"
    },
    gap: Some(
        "Selected WebSocket assertions do not compare every board/hand field or malformed upstream conversion branch; full snapshot component values are covered in game-snapshots-access."
    ),
});
