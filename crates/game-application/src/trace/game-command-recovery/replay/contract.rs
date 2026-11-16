use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "再試行処理は最初に参加者への所属を確認し、対局ごとに受理済みの ID に対応する MutationRequest 全体を比較して、元の Acceptance を返す。",
    source: trace::Source {
        path: "crates/game-application/src/actor.rs",
        needle: "fn replay(&self, request: &MutationRequest) -> Result<Option<Acceptance>, ApplicationError> {"
    },
    gap: None,
});
