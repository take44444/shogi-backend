use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "valkey-publish",
    consumer: "game-messaging",
    provider: "valkey",
    interface: trace::Interface::External,
    statement: "PUBLISH が指定チャネルへの受け入れを返した場合だけ成功と扱う。購読者の処理完了や永続保存とは区別する。",
    assumptions: &["呼出元は公開インターフェースの型と認証条件を満たす。"],
    limitations: &["下位契約の検証だけでは上位契約を証明しない。"],
    source: trace::Source {
        path: "crates/game-messaging/src/lib.rs",
        needle: "impl NotificationPublisher for ValkeyEventPublisher {"
    },
    call: Some(trace::Source {
        path: "crates/game-messaging/src/lib.rs",
        needle: "impl NotificationPublisher for ValkeyEventPublisher {"
    }),
    implementation: None,
    binding: None,
    requires: &[],
    gap: Some(
        "この境界の全入力・外部障害・競合順序は網羅しない。実外部サービスを使わないテストはその提供元の根拠に含めない。"
    )
});
