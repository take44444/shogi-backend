use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Game::play は終局後または手番の異なる操作を拒否し、合法性の判定を委譲する。次の側が王手で合法手を持たない場合だけ詰みとする。千日手と連続王手の裁定は行わない。",
    source: trace::Source {
        path: "crates/shogi-domain/src/lib.rs",
        needle: "pub fn play(&mut self, player: Player, mv: Move) -> Result<MoveOutcome, CommandError> {"
    },
    gap: Some(
        "時計、時間切れ、入玉、宣言による裁定は存在しない。テストは未対応の裁定の全事例を列挙していない。"
    ),
});
