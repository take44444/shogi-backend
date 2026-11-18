use crate::trace_support as trace;
trace::component!(trace::Component {
    description: "player-server コンポーネント",
    notes: &[
        "startup-schema-isolation/reader-process。公開読み取り実行ファイルは PLAYER_BIND_ADDR を解析し、待ち受け前に検証付き読み取りアダプターで PLAYER_GAME_DATABASE_URL へ接続する。その読み取り処理を棋譜とレーティングで共有し、Ctrl-C による正常停止後に接続を閉じる。 未検証範囲。この実行ファイルを起動し、環境変数の既定値、待ち受けアドレスの解析、待ち受け前のスキーマ拒否、Ctrl-C、読み取り接続の終了を確認する対応テストはない。プロセス内 HTTP とデータベースアダプターのテストでは、この実行ファイルの接続構成を検証しない。"
    ],
    external: &[],
    operations: &[],
    entrypoints: &[trace::EntryPoint {
        name: "公開データサービス起動",
        source: trace::Source {
            path: "apps/player-server/src/main.rs",
            needle: "async fn main() -> Result<(), Box<dyn std::error::Error>> {"
        }
    }]
});
