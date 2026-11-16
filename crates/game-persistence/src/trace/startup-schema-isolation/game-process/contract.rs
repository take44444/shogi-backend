use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "対局実行ファイルは、最初の引数、GAME_SERVER_ADDR、ループバックの既定値の順にアドレスを選ぶ。gRPC の要求を受け付ける前に書き込みスキーマへ接続し、サーバー停止後に組合せ処理へ停止を通知する。",
    source: trace::Source {
        path: "apps/game-server/src/main.rs",
        needle: "async fn main() -> Result<(), Box<dyn std::error::Error>> {"
    },
    gap: Some(
        "この実行ファイルを起動し、アドレスの優先順位、不正な設定、依存先の失敗、Ctrl-C、組合せ処理の終了を検証する対応テストはない。組合せ処理の振る舞いは matchmaking に属し、書き込みアダプターのテストはプロセスのライフサイクルを証明しない。"
    ),
});
