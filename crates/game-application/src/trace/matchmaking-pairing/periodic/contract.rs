use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "実行ファイルは遅延した tick を読み飛ばす一秒間隔を使い、開始プールを交互に替え、最大 32 回の tick を試す。二つのプールが空、または最初の失敗となった時点でバッチを終了する。",
    source: trace::Source {
        path: "apps/game-server/src/main.rs",
        needle: "let matcher = tokio::spawn(async move {"
    },
    gap: Some(
        "このテストは実際に 30 秒待たず、DB の時刻を過去に変更する。両プールの公平性、32 回の試行上限、エラー記録と復旧、Ctrl-C による停止は検証しない。"
    ),
});
