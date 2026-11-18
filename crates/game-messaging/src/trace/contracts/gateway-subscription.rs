use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "gateway-subscription",
    consumer: "realtime-gateway",
    provider: "game-messaging",
    interface: trace::Interface::RustApi,
    statement: "指定した対局チャネルを購読し、正規の通知を上限付きキューで渡す。接続・復号の失敗やキュー溢れは購読の失敗として返す。シーケンスの欠落判定は利用側が行う。",
    assumptions: &["呼出元は公開インターフェースの型と認証条件を満たす。"],
    limitations: &["下位契約の検証だけでは上位契約を証明しない。"],
    source: trace::Source {
        path: "crates/game-messaging/src/lib.rs",
        needle: "pub async fn subscribe("
    },
    call: Some(trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "subscription = ValkeySubscription::subscribe(&gateway.valkey, &game, gateway.limits.queue) => subscription,"
    }),
    implementation: Some(trace::Source {
        path: "crates/game-messaging/src/lib.rs",
        needle: "pub async fn subscribe("
    }),
    binding: None,
    requires: &[trace::Dependency {
        contract: "game-messaging/valkey-subscription",
        reason: "通知の取得と接続喪失の観測に、実際の Valkey 購読を利用する。"
    }],
    gap: Some(
        "この境界の全入力・外部障害・競合順序は網羅しない。実外部サービスを使わないテストはその提供元の根拠に含めない。"
    )
});
