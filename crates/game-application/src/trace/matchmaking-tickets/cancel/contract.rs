use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "取消は有効な待機チケットを遷移させるか、所有者に属する取消記録を挿入する。組合せ済みまたは終端状態のチケットは、対局を取り消さずそのまま返す。",
    source: trace::Source {
        path: "crates/game-persistence/src/matchmaking.rs",
        needle: "async fn cancel(&self, user: &UserId, id: TicketId) -> Result<Ticket, MError> {"
    },
    gap: None,
});
