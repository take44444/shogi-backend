use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "GameRecords::search passes the typed Search to its RecordRepository and returns its result.",
    source: trace::Source {
        path: "crates/game-record-application/src/lib.rs",
        needle: "pub async fn search(&self, search: &Search) -> Result<Page, Error> {"
    },
    gap: Some(
        "No independent test asserts unchanged forwarding of arbitrary Search values and repository results."
    ),
});
