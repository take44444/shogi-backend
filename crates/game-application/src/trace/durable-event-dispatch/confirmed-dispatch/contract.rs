use crate::trace_support::{self as trace, Contract};
trace::contract!(Contract {
    statement: "OutboxPublisher は配信してから処理済みにする。Valkey アダプターは有効な非負整数の PUBLISH 応答だけを受け入れ、取消またはタイムアウトした接続を無効化する。",
    source: trace::Source {
        path: "crates/game-application/src/events.rs",
        needle: "pub async fn publish_next(&mut self) -> Result<bool, DispatchError> {"
    },
    gap: Some(
        "通信テストは制御可能な TCP RESP のテスト用実装とテスト用リポジトリを使う。起動済み Valkey や購読者は使わない。"
    )
});
