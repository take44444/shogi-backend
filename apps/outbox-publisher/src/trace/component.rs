use crate::trace_support as trace;
trace::component!(trace::Component {
    description: "outbox-publisher コンポーネント",
    notes: &[
        "durable-event-dispatch/publisher-process。実行ファイルはキューの所有権を取得し、キューを破棄する前にランタイムを破棄する。待機時に遅延を入れ、Ctrl-Cを扱う配信ループを実行する。 未検証範囲。対応付けたテストには、この実行ファイルを起動するものや、環境変数の解析、100ミリ秒の待機遅延、エラー終了、Ctrl-Cの処理、ランタイムの終了処理、プロセスの再起動時の動作を検証するものはない。"
    ],
    external: &[],
    operations: &[],
    entrypoints: &[trace::EntryPoint {
        name: "Outbox 配信プロセス",
        source: trace::Source {
            path: "apps/outbox-publisher/src/main.rs",
            needle: "fn main() -> Result<(), Error> {"
        }
    }]
});
