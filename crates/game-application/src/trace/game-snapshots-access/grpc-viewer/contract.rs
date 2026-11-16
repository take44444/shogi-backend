use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "GetSnapshotForViewer と CheckViewAccess は対局・閲覧者 ID を検証し、呼び出し元の閲覧者の申告を渡してアプリケーションエラーを変換するが、その申告自体は認証しない。",
    source: trace::Source {
        path: "crates/game-grpc/src/service.rs",
        needle: "async fn get_snapshot_for_viewer("
    },
    gap: Some(
        "閲覧者向けの両 RPC で、不正な閲覧者 ID と対局 ID の境界をそれぞれ検証してはいない。"
    ),
});
