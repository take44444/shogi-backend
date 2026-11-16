use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "アダプターは合法手をすべて生成し、その集合から型付きリクエストを検索し、失敗可能で原子的なルールエンジンの更新を適用する。",
    source: trace::Source {
        path: "crates/shogi-domain/src/adapter.rs",
        needle: "pub(crate) fn play(&mut self, mv: Move) -> Result<(), CommandError> {"
    },
    gap: Some(
        "代表的なテスト用局面で、対応する各ルール系統を直接検証する。到達可能な全局面や、ネットワーククライアント経由の全ルールは扱わない。"
    ),
});
