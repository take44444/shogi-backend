use crate::trace_support::{self as trace, Contract};
trace::contract!(Contract {
    statement: "実行ファイルはキューの所有権を取得し、キューを破棄する前にランタイムを破棄する。待機時に遅延を入れ、Ctrl-Cを扱う配信ループを実行する。",
    source: trace::Source {
        path: "apps/outbox-publisher/src/main.rs",
        needle: "async fn run(queue: &mut PostgresOutbox, valkey: &str) -> Result<(), Error> {"
    },
    gap: Some(
        "対応付けたテストには、この実行ファイルを起動するものや、環境変数の解析、100ミリ秒の待機遅延、エラー終了、Ctrl-Cの処理、ランタイムの終了処理、プロセスの再起動時の動作を検証するものはない。"
    )
});
