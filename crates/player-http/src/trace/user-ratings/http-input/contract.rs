use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "レート取得ハンドラーは Path の復号と UserId の検証が成功するまで repository を呼ばない。",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "async fn ratings(State(app): State<App>, path: Result<Path<String>, PathRejection>) -> Response {"
    },
    gap: Some("このテストには ID の全長境界や、すべての不正なパスの列挙はない。"),
});
