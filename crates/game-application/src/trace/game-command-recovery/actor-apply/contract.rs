use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "変更処理は状態を更新し、予想シーケンスの確認より先に再試行を扱う。複製した候補局面と渡すイベントをステージし、コミット確認後にだけメモリー内の候補を公開する。",
    source: trace::Source {
        path: "crates/game-application/src/actor.rs",
        needle: "async fn apply(&mut self, pending: MutationRequest) -> Result<Acceptance, ApplicationError> {"
    },
    gap: None,
});
