use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "PostgresGameRecords::connect は既存スキーマを初期化せず検証し、接続プールを四つに制限する。トランザクションを読み取り専用に設定し、接続取得・文実行・ロックのタイムアウトを二秒にする。",
    source: trace::Source {
        path: "crates/game-record-persistence/src/lib.rs",
        needle: "    pub async fn connect(url: &str) -> Result<Self, Error> {"
    },
    gap: Some(
        "lock_timeout の設定は確認しているが、実際にロック待ちでタイムアウトする別の事例は対応付けられていない。"
    ),
});
