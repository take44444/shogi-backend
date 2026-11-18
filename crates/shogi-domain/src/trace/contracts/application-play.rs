use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "application-play",
    consumer: "game-application",
    provider: "shogi-domain",
    interface: trace::Interface::RustApi,
    statement: "Game::play は終局後または手番の異なる操作を拒否し、合法性の判定を委譲する。次の側が王手で合法手を持たない場合だけ詰みとする。千日手と連続王手の裁定は行わない。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/shogi-domain/src/lib.rs",
        needle: "pub fn play(&mut self, player: Player, mv: Move) -> Result<MoveOutcome, CommandError> {"
    },
    call: Some(trace::Source {
        path: "crates/game-application/src/actor.rs",
        needle: "game.play(side, movement)"
    }),
    implementation: Some(trace::Source {
        path: "crates/shogi-domain/src/lib.rs",
        needle: "pub fn play(&mut self, player: Player, mv: Move) -> Result<MoveOutcome, CommandError> {"
    }),
    binding: None,
    requires: &[],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
