use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "UserId::new は空でなく 128 UTF-8 バイト以内で Unicode 空白を含まない内部 ID を保持する。",
    source: trace::Source {
        path: "crates/user-id/src/lib.rs",
        needle: "    pub fn new(value: impl Into<String>) -> Result<Self, InvalidId> {"
    },
    gap: None,
});
