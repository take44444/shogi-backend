use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "再読み込みは、永続履歴の読み取りと検証の前にローカル状態を消去する。照合に失敗すると、後の再読み込みで復元できるまで利用不能のままとなる。",
    source: trace::Source {
        path: "crates/game-application/src/actor.rs",
        needle: "async fn reload(&mut self) -> Result<(), ApplicationError> {"
    },
    gap: Some(
        "障害注入アダプターは代表的な結果経路を制御するが、任意のプロセス実行やクラッシュの割り込み順序は証明しない。"
    ),
});
