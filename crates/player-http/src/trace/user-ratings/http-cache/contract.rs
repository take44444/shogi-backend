use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "respond は応答の Cache-Control ヘッダーを no-store にする。",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "fn respond<T: Serialize>(result: Result<T, Error>) -> Response {"
    },
    gap: Some("登録した不正 ID テストは 400 の Cache-Control を確認しない。"),
});
