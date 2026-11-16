use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "tick は永続的な参加受け入れを直列化し、選択したチケットとレーティング状態をロックする。待機後に適合性を再確認し、対局・レーティング予約枠・二枚の Playing チケットを原子的に作成する。",
    source: trace::Source {
        path: "crates/game-persistence/src/matchmaking.rs",
        needle: "async fn tick(&self, pool: Pool) -> Result<Option<GameId>, MError> {"
    },
    gap: Some(
        "選択したテストは、両チケットの先後割り当てと、コミットした対局の参加者割り当てを比較しない。実行ファイルのテストはスナップショットのユーザーを確認するが、チケットの先後との一致は比較しない。"
    ),
});
