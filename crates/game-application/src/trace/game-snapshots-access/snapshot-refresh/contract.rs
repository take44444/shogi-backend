use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "通常のスナップショット要求は、アクターの状態を返す前に永続バージョンを確認する。ストレージ失敗時にキャッシュした局面へ切り替えることはない。",
    source: trace::Source {
        path: "crates/game-application/src/actor.rs",
        needle: "async fn refresh(&mut self) -> Result<(), ApplicationError> {"
    },
    gap: None,
});
