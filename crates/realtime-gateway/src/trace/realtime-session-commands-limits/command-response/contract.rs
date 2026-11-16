use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "command_replyは受理結果とgRPCのコードを、accepted、invalid_input、not_participant、request_conflict、stale_seq・actual_seq、rejected、outcome_unknownへ対応付ける。上流のアクセス不可・破損・内部エラーでは接続を閉じる。",
    source: trace::Source {
        path: "crates/realtime-gateway/src/game.rs",
        needle: "pub(crate) fn command_reply(id: &str, result: Result<proto::Acceptance, Status>) -> CommandOutcome {"
    },
    gap: Some(
        "選択したゲートウェイのテストには、stale_seqのメタデータ、request_conflict、rejected、outcome_unknown、上流の全ステータス・不正な結果から接続終了への対応を直接検証するものはない。"
    ),
});
