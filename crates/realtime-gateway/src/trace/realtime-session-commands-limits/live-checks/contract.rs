use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "command_workerは、session.userを使って上流のコマンドを呼ぶ前に、セッションと現在の対局へのアクセスを再確認する。",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "async fn command_worker("
    },
    gap: Some(
        "応答前の再確認に関する競合を独立して検証するテストや、上流の変更処理の期限と結果を確認するテストはない。"
    ),
});
