use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Cancellation transitions a live waiting ticket or inserts an owner-scoped tombstone; matched/terminal tickets are returned without cancelling a game.",
    source: trace::Source {
        path: "crates/game-persistence/src/matchmaking.rs",
        needle: "async fn cancel(&self, user: &UserId, id: TicketId) -> Result<Ticket, MError> {"
    },
    gap: None,
});
