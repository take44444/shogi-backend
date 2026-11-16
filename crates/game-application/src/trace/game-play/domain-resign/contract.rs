use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "進行中はどちらの参加者も投了できる。結果は反対側を勝者とし、盤面と手番を保持して以後の操作を拒否する。",
    source: trace::Source {
        path: "crates/shogi-domain/src/lib.rs",
        needle: "pub fn resign(&mut self, player: Player) -> Result<GameResult, CommandError> {"
    },
    gap: None,
});
