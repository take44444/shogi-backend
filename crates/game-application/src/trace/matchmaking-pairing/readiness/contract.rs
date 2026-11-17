use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "準備完了は所有者ごとに永続化され、期限内に双方が揃ったときだけ対局・レーティング予約枠・両者の Playing 遷移を原子的に作る。ロック待ち後も期限を確認し、期限切れは両者を Voided にして参加枠を解放する。再送で期限は延長しない。",
    source: trace::Source {
        path: "crates/game-persistence/src/matchmaking.rs",
        needle: "async fn ready(&self, user: &UserId, id: TicketId) -> Result<Ticket, MError> {"
    },
    gap: Some(
        "期限 15 秒は初期方針であり、利用実績に基づく値ではない。高負荷での準備確認や期限切れ処理の待ち時間は測定していない。"
    ),
});
