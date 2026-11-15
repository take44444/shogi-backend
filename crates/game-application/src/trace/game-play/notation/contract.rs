use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Move parsing accepts exactly board USI with optional trailing + or uppercase unpromoted hand-piece drops, with board-bounded squares and no whitespace/extra bytes.",
    source: trace::Source {
        path: "crates/shogi-domain/src/notation.rs",
        needle: "impl FromStr for Move {"
    },
    gap: None,
});
