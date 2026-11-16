use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "ドメインの局面と複製した対局は、それぞれ独立して盤面・持ち駒・終局結果を所有する。",
    source: trace::Source {
        path: "crates/shogi-domain/src/lib.rs",
        needle: "pub fn position(&self) -> PositionView {"
    },
    gap: None,
});
