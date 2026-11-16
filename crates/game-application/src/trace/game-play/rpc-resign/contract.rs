use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Resign は対局・リクエスト・ユーザー ID と必須の予想シーケンスを検証し、ゲートウェイで認証済みの参加者の申告をアプリケーションへ委譲する。",
    source: trace::Source {
        path: "crates/game-grpc/src/service.rs",
        needle: "async fn resign("
    },
    gap: None,
});
