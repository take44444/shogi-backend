use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Either participant side may resign while ongoing; result names the opposite side, retains position/turn and rejects later commands.",
    source: trace::Source {
        path: "crates/shogi-domain/src/lib.rs",
        needle: "pub fn resign(&mut self, player: Player) -> Result<GameResult, CommandError> {"
    },
    gap: None,
});
