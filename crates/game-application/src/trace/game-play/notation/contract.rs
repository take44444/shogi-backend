use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "着手の解析は、末尾に任意の + を持つ盤上 USI、または大文字の生駒による駒打ちだけを受け入れる。升目は盤内に限り、空白や余分なバイトは許可しない。",
    source: trace::Source {
        path: "crates/shogi-domain/src/notation.rs",
        needle: "impl FromStr for Move {"
    },
    gap: None,
});
