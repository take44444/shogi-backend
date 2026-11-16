use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "未評価のレーティングは実効値 1500 とする。許容差は 100 から始まり、30 秒が完全に経過するごとに 50 増え、上限は 400 となる。広い整数型で計算する。",
    source: trace::Source {
        path: "crates/game-application/src/matchmaking.rs",
        needle: "pub fn window(enqueued_at: i64, now: i64) -> i128 {"
    },
    gap: None,
});
