use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "notification-publisher",
    consumer: "game-application",
    provider: "game-messaging",
    interface: trace::Interface::Port,
    statement: "固定したイベントを指定チャネルへ送信し、通信先の受け入れ結果を返す。購読者の処理完了や永続保存を保証しない。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/game-application/src/events.rs",
        needle: "pub trait NotificationPublisher: Send {"
    },
    call: Some(trace::Source {
        path: "crates/game-application/src/events.rs",
        needle: "pub async fn publish_next(&mut self) -> Result<bool, DispatchError> {"
    }),
    implementation: Some(trace::Source {
        path: "crates/game-messaging/src/lib.rs",
        needle: "impl NotificationPublisher for ValkeyEventPublisher {"
    }),
    binding: Some(trace::Source {
        path: "apps/outbox-publisher/src/main.rs",
        needle: "let mut publisher = OutboxPublisher::new(queue, transport);"
    }),
    requires: &[trace::Dependency {
        contract: "game-messaging/valkey-publish",
        reason: "イベント配信の受け入れ確認を Valkey の PUBLISH 結果へ対応付ける。"
    }],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
