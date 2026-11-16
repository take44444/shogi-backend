use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "現在状態とチケットの取得処理は、トランザクション内で古い状態を失効させ、認証済み所有者を対象に問い合わせる。現在状態の取得は待機中または対局中の参加だけを返す。",
    source: trace::Source {
        path: "crates/game-persistence/src/matchmaking.rs",
        needle: "async fn current(&self, user: &UserId) -> Result<Participation, MError> {"
    },
    gap: None,
});
