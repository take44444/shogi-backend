use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "詳細取得ハンドラーはパスの ID を検証し、共有の処理許可を使い、リポジトリの失敗を詳細を伏せた no-store 付き HTTP 応答へ変換する。",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "async fn detail(State(app): State<App>, path: Result<Path<String>, PathRejection>) -> Response {"
    },
    gap: Some(
        "実ルーターのテスト用データには非公開の対局を作らない。非公開の場合の動作はリポジトリのテストと共有エラー変換で裏付ける。共有の受け入れ件数と応答バイト数の上限は public-record-search に対応付けている。"
    ),
});
