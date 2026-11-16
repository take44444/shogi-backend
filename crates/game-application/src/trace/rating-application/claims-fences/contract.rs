use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "永続リポジトリはロック中のジョブを読み飛ばして実行可能なジョブを取得する。処理前に所有者・取得世代・有効なリースを確認し、状態のロック待ち後に原子的適用を完了する際にも再確認する。",
    source: trace::Source {
        path: "crates/game-persistence/src/rating.rs",
        needle: "impl RatingJobRepository for SeaOrmRatingJobs {"
    },
    gap: None,
});
