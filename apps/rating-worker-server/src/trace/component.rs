use crate::trace_support as trace;
trace::component!(trace::Component {
    description: "rating-worker-server コンポーネント",
    notes: &[
        "rating-application/worker-process。ワーカー実行ファイルは、範囲を制限した方針とポーリング値を解析する。永続リポジトリを起動し、定期実行または明示的な retry-quarantined モードを実行してから接続を閉じる。 未検証範囲。この CLI を起動し、実際の引数と環境変数の解析、ポーリング間隔 0 の拒否、Ctrl-C、定期実行の時機、プロセス停止時のリポジトリ終了を確認する対応テストはない。"
    ],
    external: &[],
    operations: &[],
    entrypoints: &[trace::EntryPoint {
        name: "レートジョブのポーリングと隔離復旧 CLI",
        source: trace::Source {
            path: "apps/rating-worker-server/src/main.rs",
            needle: "async fn main() -> Result<(), Box<dyn std::error::Error>> {"
        }
    }]
});
