use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The transaction checks existing owner/UUID first, then global active participation and per-pool pending rating reservations; admitted/refused state is committed for later retries.",
    source: trace::Source {
        path: "crates/game-persistence/src/matchmaking.rs",
        needle: "async fn join(&self, user: UserId, id: TicketId, pool: Pool) -> Result<JoinOutcome, MError> {"
    },
    gap: None,
});
