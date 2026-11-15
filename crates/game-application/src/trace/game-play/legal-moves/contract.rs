use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The adapter generates complete legal moves, looks up the typed request in that set and applies a fallible atomic rules-engine update.",
    source: trace::Source {
        path: "crates/shogi-domain/src/adapter.rs",
        needle: "pub(crate) fn play(&mut self, mv: Move) -> Result<(), CommandError> {"
    },
    gap: Some(
        "Representative direct fixtures cover each supported rule family, not every reachable position or every rule through a network client."
    ),
});
