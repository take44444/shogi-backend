use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Selection excludes expired waiting, same owner and other pools, orders anchors by enqueue/UUID/owner and opponents by difference/age/UUID/owner.",
    source: trace::Source {
        path: "crates/game-application/src/matchmaking.rs",
        needle: "pub fn select_pair(tickets: &[Ticket], now: i64) -> Option<(&Ticket, &Ticket)> {"
    },
    gap: None,
});
