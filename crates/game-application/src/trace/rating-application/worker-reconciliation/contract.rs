use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "RatingWorker は不正入力と適用時の破損を隔離する。利用不能となった適用を一度再試行して不確かなコミットを照合し、リポジトリの結果を報告する。",
    source: trace::Source {
        path: "crates/game-application/src/rating.rs",
        needle: "pub async fn tick(&self, owner: &WorkerId) -> Result<TickOutcome, RatingWorkerError> {"
    },
    gap: Some(
        "選択したエラー経路は検証しているが、全分岐を独立に網羅する検証や、WorkerId・WorkerPolicy・ClaimedRatingJob の専用コンストラクターテストはない。"
    ),
});
