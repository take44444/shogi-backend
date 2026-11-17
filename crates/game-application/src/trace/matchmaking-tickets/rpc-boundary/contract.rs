use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "六つの組合せ RPC は所有者と UUID を解析し、対応するプールと正のハートビートを検証する。所有者に限定した操作を委譲し、チケット状態を直列化する。",
    source: trace::Source {
        path: "crates/game-grpc/src/service.rs",
        needle: "async fn join_matchmaking("
    },
    gap: Some(
        "すべての RPC で不正な所有者と UUID の事例を列挙してはいない。専用の TCP テストはハートビートの成功を検証しないが、ブラウザーテストは RPC 経由で成功するハートビートを検証する。"
    ),
});
