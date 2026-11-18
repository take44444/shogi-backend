use crate::trace_support as trace;
trace::component!(trace::Component {
    description: "game-messaging コンポーネント",
    notes: &[
        "durable-event-dispatch/wire-format。メッセージングアダプターは、スキーマのバージョンが1のJSONにイベント識別子を明示し、対局IDの全バイトから配信先チャネルを生成する。 未検証範囲。購読者への配信、購読のライフサイクル、通知の拒否は realtime-gameplay 側で対応付けており、これらの配信例では検証しない。"
    ],
    external: &[],
    operations: &[],
    entrypoints: &[]
});
