use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Metadata requires distinct participants, a terminal sequence, valid storage scalar bounds and finish-count relations appropriate to moves or resignation.",
    source: trace::Source {
        path: "crates/game-record-application/src/lib.rs",
        needle: "impl Metadata {"
    },
    gap: None,
});
