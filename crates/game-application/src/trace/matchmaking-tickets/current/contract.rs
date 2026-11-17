use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "現在参加の取得は、トランザクション内で古い状態を失効させ、所有者の待機中・準備待ち・対局中の参加だけを返す。待機期限は延長せず、参加がなければ Idle を返す。",
    source: trace::Source {
        path: "crates/game-persistence/src/matchmaking.rs",
        needle: "async fn current(&self, user: &UserId) -> Result<Participation, MError> {"
    },
    gap: None,
});
