use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "application-resign",
    consumer: "game-application",
    provider: "shogi-domain",
    interface: trace::Interface::RustApi,
    statement: "進行中はどちらの参加者も投了できる。結果は反対側を勝者とし、盤面と手番を保持して以後の操作を拒否する。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/shogi-domain/src/lib.rs",
        needle: "pub fn resign(&mut self, player: Player) -> Result<GameResult, CommandError> {"
    },
    call: Some(trace::Source {
        path: "crates/game-application/src/actor.rs",
        needle: "game.resign(side).map_err(ApplicationError::Domain)?;"
    }),
    implementation: Some(trace::Source {
        path: "crates/shogi-domain/src/lib.rs",
        needle: "pub fn resign(&mut self, player: Player) -> Result<GameResult, CommandError> {"
    }),
    binding: None,
    requires: &[],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
