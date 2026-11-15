use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "GameRecords::detail passes the typed GameId to its repository and returns the result.",
    source: trace::Source {
        path: "crates/game-record-application/src/lib.rs",
        needle: "pub async fn detail(&self, id: &GameId) -> Result<Detail, Error> {"
    },
    gap: Some("There is no independent unchanged-forwarding test for arbitrary IDs and results."),
});
