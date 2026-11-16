use crate::trace_support::{self as trace, Contract};
trace::contract!(Contract {
    statement: "PostgresOutbox は自動再接続しない一つのアドバイザリーロック付きセッションを使い、順序付き読み取りと対象行だけの冪等な処理済み更新を行う。未対応の固定イベント符号化を拒否し、通信待ち中はトランザクションを開いたままにしない。",
    source: trace::Source {
        path: "crates/game-persistence/src/outbox.rs",
        needle: "impl OutboxRepository for PostgresOutbox {"
    },
    gap: Some(
        "配信呼び出しの境界に置くテスト用実装で、データベース状態と後続の書き込みを観測する。実際のブローカーで待機する、全コンポーネントを通したテストではない。"
    )
});
