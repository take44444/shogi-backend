use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "アカウント起動処理は ServerConfig を読み込み、ストアへ接続し、Google の接続情報を取得してから待ち受ける。Compose はアカウント専用ストレージ、独立した読み取りプロセス、データベース認証情報を持たないゲートウェイを接続する。",
    source: trace::Source {
        path: "apps/user-server/src/main.rs",
        needle: "async fn main() -> Result<(), Box<dyn std::error::Error>> {"
    },
    gap: Some(
        "すべてのサービスを起動し、ServerConfig::from_env、実際の Google 情報取得の可用性、Ctrl-C と接続終了、アプリケーションの準備完了を検証する対応 Rust テストはない。既存の python3 scripts/test-deployment.py は Docker と PostgreSQL を使い、Compose の配置構成、ループバックへの公開、オリジンによる経路選択、読み取り権限、アカウントストレージへのアクセス拒否を検査する。この Cargo 以外のコマンドは traceability run の実行対象に含まれない。"
    ),
});
