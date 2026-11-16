use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "アプリケーションとドメインのエラーを安定した gRPC 区分へ変換する。通常の拒否は FailedPrecondition、非参加者は PermissionDenied、古いシーケンスは actual-seq 付き Aborted、ルール処理や基盤の内部エラーは Internal とする。",
    source: trace::Source {
        path: "crates/game-grpc/src/conversion.rs",
        needle: "pub(crate) fn application_status(error: ApplicationError) -> Status {"
    },
    gap: None,
});
