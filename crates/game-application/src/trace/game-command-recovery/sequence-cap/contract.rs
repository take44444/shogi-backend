use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "next_sequence は i64::MAX から先への更新を拒否する。上限到達後の更新を試す前に、古い予想シーケンスを確認する。",
    source: trace::Source {
        path: "crates/game-application/src/actor.rs",
        needle: "fn next_sequence(seq: u64) -> Result<u64, ApplicationError> {"
    },
    gap: Some(
        "テストは最大状態を合成する。ルールエンジンが実際に何十億手もの着手を処理できることは証明しない。"
    ),
});
