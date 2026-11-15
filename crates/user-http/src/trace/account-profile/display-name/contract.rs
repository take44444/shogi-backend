use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "DisplayName::new は trim 後の文字列を保持し、空文字・80 Unicode 文字超過・制御文字を InvalidInput とする。",
    source: trace::Source {
        path: "crates/user-application/src/lib.rs",
        needle: "    pub fn new(value: String) -> Result<Self, Error> {"
    },
    gap: None,
});
