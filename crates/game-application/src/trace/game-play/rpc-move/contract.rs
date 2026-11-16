use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "MakeMove は対局・リクエスト・ユーザー ID を検証し、expected_seq を必須とし、USI を解析して型付きのアプリケーション操作を渡す。",
    source: trace::Source {
        path: "crates/game-grpc/src/service.rs",
        needle: "async fn make_move("
    },
    gap: None,
});
