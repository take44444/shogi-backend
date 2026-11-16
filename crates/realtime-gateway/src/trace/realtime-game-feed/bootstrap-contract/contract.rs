use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "run_connectionは閲覧者用スナップショットの取得前に購読と通知転送を開始する。スナップショットを検証して最初にキューへ入れ、その連番でカーソルを初期化し、蓄積した通知をカーソルに通す。",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "async fn run_connection("
    },
    gap: Some(
        "稼働中のブローカーの切断・再接続を通して、購読喪失の全経路を確認する端から端までの検証はない。再接続では永続イベントログを再送しない。"
    ),
});
