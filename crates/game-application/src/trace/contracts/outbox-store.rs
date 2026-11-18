use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "outbox-store",
    consumer: "game-application",
    provider: "game-persistence",
    interface: trace::Interface::Port,
    statement: "PostgresOutbox は自動再接続しない一つのアドバイザリーロック付きセッションを使い、順序付き読み取りと対象行だけの冪等な処理済み更新を行う。未対応の固定イベント符号化を拒否し、通信待ち中はトランザクションを開いたままにしない。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/game-application/src/events.rs",
        needle: "pub trait OutboxRepository: Send {"
    },
    call: Some(trace::Source {
        path: "crates/game-application/src/events.rs",
        needle: "pub async fn publish_next(&mut self) -> Result<bool, DispatchError> {"
    }),
    implementation: Some(trace::Source {
        path: "crates/game-persistence/src/outbox.rs",
        needle: "impl OutboxRepository for PostgresOutbox {"
    }),
    binding: Some(trace::Source {
        path: "apps/outbox-publisher/src/main.rs",
        needle: "let mut publisher = OutboxPublisher::new(queue, transport);"
    }),
    requires: &[],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
