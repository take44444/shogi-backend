use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "UserId は 1〜128 UTF-8 バイトで Unicode 空白を含まない ID を保持する。",
    source: trace::Source {
        path: "crates/user-id/src/lib.rs",
        needle: "pub fn new(value: impl Into<String>) -> Result<Self, InvalidId> {"
    },
    gap: None,
});
