use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "valkey-subscription",
    consumer: "game-messaging",
    provider: "valkey",
    interface: trace::Interface::External,
    statement: "指定した対局チャネルの購読を確立し、到着したメッセージをキューへ渡す。キューの溢れまたは接続喪失はその購読の終端として観測できる。",
    assumptions: &["呼出元は公開インターフェースの型と認証条件を満たす。"],
    limitations: &["下位契約の検証だけでは上位契約を証明しない。"],
    source: trace::Source {
        path: "crates/game-messaging/src/lib.rs",
        needle: "pub async fn subscribe("
    },
    call: Some(trace::Source {
        path: "crates/game-messaging/src/lib.rs",
        needle: "pub async fn subscribe("
    }),
    implementation: None,
    binding: None,
    requires: &[],
    gap: Some(
        "この境界の全入力・外部障害・競合順序は網羅しない。実外部サービスを使わないテストはその提供元の根拠に含めない。"
    )
});
