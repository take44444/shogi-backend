use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "レート取得ハンドラーは処理枠取得の失敗と repository の全失敗を Unavailable に変換し、503 を返す。",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "async fn ratings(State(app): State<App>, path: Result<Path<String>, PathRejection>) -> Response {"
    },
    gap: Some("処理枠が尽きたレート取得の応答を確認するテストは登録していない。"),
});
