use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "AuthClient::recheck は保存した Cookie で再認証し、接続開始時の UserId と同じであることを要求する。",
    source: trace::Source {
        path: "crates/realtime-gateway/src/auth.rs",
        needle: "    pub(crate) async fn recheck("
    },
    gap: Some(
        "別 UserId を返す応答と idle 接続の定期失効確認は直接確認しない。再確認は既存のコマンド・通知処理時の証拠である。"
    ),
});
