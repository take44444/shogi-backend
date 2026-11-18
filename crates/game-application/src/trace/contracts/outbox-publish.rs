use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "outbox-publish",
    consumer: "outbox-publisher",
    provider: "game-application",
    interface: trace::Interface::RustApi,
    statement: "OutboxPublisher は配信してから処理済みにする。Valkey アダプターは有効な非負整数の PUBLISH 応答だけを受け入れ、取消またはタイムアウトした接続を無効化する。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/game-application/src/events.rs",
        needle: "pub async fn publish_next(&mut self) -> Result<bool, DispatchError> {"
    },
    call: Some(trace::Source {
        path: "apps/outbox-publisher/src/main.rs",
        needle: "delay = if publisher.publish_next().await? {"
    }),
    implementation: Some(trace::Source {
        path: "crates/game-application/src/events.rs",
        needle: "pub async fn publish_next(&mut self) -> Result<bool, DispatchError> {"
    }),
    binding: None,
    requires: &[
        trace::Dependency {
            contract: "game-application/outbox-store",
            reason: "提供元はこの直接の依存先を利用して、上位の応答と状態の保証を満たす。"
        },
        trace::Dependency {
            contract: "game-application/notification-publisher",
            reason: "提供元はこの直接の依存先を利用して、上位の応答と状態の保証を満たす。"
        }
    ],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
