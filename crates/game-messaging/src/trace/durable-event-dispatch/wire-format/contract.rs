use crate::trace_support::{self as trace, Contract};
trace::contract!(Contract {
    statement: "メッセージングアダプターは、スキーマのバージョンが1のJSONにイベント識別子を明示し、対局IDの全バイトから配信先チャネルを生成する。",
    source: trace::Source {
        path: "crates/game-messaging/src/lib.rs",
        needle: "fn encode(event: &CommittedEvent) -> Result<Vec<u8>, PublishError> {"
    },
    gap: Some(
        "購読者への配信、購読のライフサイクル、通知の拒否は realtime-gameplay 側で対応付けており、これらの配信例では検証しない。"
    )
});
