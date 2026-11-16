use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "トランザクションは、既存の所有者と UUID、全体の有効な参加、プールごとのレーティング適用待ち予約枠の順に確認する。参加受け入れまたは拒否の状態をコミットし、後の再試行に使う。",
    source: trace::Source {
        path: "crates/game-persistence/src/matchmaking.rs",
        needle: "async fn join(&self, user: UserId, id: TicketId, pool: Pool) -> Result<JoinOutcome, MError> {"
    },
    gap: None,
});
