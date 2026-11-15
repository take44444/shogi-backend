use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Domain positions and cloned games own their board, hands and terminal result independently.",
    source: trace::Source {
        path: "crates/shogi-domain/src/lib.rs",
        needle: "pub fn position(&self) -> PositionView {"
    },
    gap: None,
});
