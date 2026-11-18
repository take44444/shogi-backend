use crate::trace_support as trace;
trace::component!(trace::Component {
    description: "game-server コンポーネント",
    notes: &[
        "matchmaking-pairing/periodic。実行ファイルは遅延した tick を読み飛ばす一秒間隔を使い、開始プールを交互に替え、最大 32 回の tick を試す。二つのプールが空、または最初の失敗となった時点でバッチを終了する。 未検証範囲。このテストは実際に 30 秒待たず、DB の時刻を過去に変更する。両プールの公平性、32 回の試行上限、エラー記録と復旧、Ctrl-C による停止は検証しない。",
        "startup-schema-isolation/game-process。対局実行ファイルは、最初の引数、GAME_SERVER_ADDR、ループバックの既定値の順にアドレスを選ぶ。gRPC の要求を受け付ける前に書き込みスキーマへ接続し、サーバー停止後に組合せ処理へ停止を通知する。 未検証範囲。この実行ファイルを起動し、アドレスの優先順位、不正な設定、依存先の失敗、Ctrl-C、組合せ処理の終了を検証する対応テストはない。組合せ処理の振る舞いは matchmaking に属し、書き込みアダプターのテストはプロセスのライフサイクルを証明しない。"
    ],
    external: &[],
    operations: &[],
    entrypoints: &[trace::EntryPoint {
        name: "待機チケットの周期的マッチング",
        source: trace::Source {
            path: "apps/game-server/src/main.rs",
            needle: "let matcher = tokio::spawn(async move {"
        }
    }]
});
