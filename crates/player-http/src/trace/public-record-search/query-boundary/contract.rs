use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "RawSearch はフィールドを型付きの値として解析し、リポジトリへアクセスする前に不正な値、未知の値、重複したクエリー値を拒否する。",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "fn parse(self) -> Result<Search, Error> {"
    },
    gap: None,
});
