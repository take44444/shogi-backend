use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "サービスの起動設定・スキーマ互換性・DB 分離",
    source: trace::Source {
        path: "crates/game-persistence/src/schema.rs",
        needle: "pub(crate) async fn initialize(db: &DatabaseConnection) -> Result<(), SchemaError> {"
    },
    flow: &[
        "実行ファイルの設定は、アプリケーション起動前にアカウント・対局データベース、ブラウザーのオリジン、サービスの接続先を選ぶ。",
        "アカウント永続層は自身のスキーマを初期化または検証する。対局の書き込みプロセスとワーカーは、空の対局データベースを初期化するか、スキーマのバージョンが 7 であることを検証する。",
        "公開読み取り処理は互換性のある対局スキーマが既に存在することを必須とし、独立した SELECT 専用の認証情報で、有界な読み取り専用接続プールを使う。",
        "Compose と初期化 SQL は、独立したアカウントストレージと認証情報を持たないゲートウェイを用意する。実行時の配置検査は、Cargo 以外の別の検査群で行う。"
    ],
    guarantees: &[
        trace::Guarantee {
            id: "browser-config",
            statement: "BrowserConfig は余分な要素のない HTTPS オリジンを必須とする。HTTP はループバックを使う開発モードを明示した場合だけ許可し、Google の正確なコールバック URL を導出する。",
            contracts: &["contract-browser-config"]
        },
        trace::Guarantee {
            id: "account-schema",
            statement: "アカウント永続層は空のデータベースを初期化し、互換性のあるスキーマとデータを保持する。検証した非互換スキーマや台帳定義には処理を進めず失敗する。",
            contracts: &["contract-account-schema"]
        },
        trace::Guarantee {
            id: "game-schema",
            statement: "対局の起動処理は空のデータベースにだけバージョン 7 のスキーマを初期化し、必須定義やバージョンの非互換を、検証対象のスキーマとデータを移行も削除もせず拒否する。",
            contracts: &["contract-game-schema"]
        },
        trace::Guarantee {
            id: "bounded-public-reader",
            statement: "公開読み取り処理はスキーマを作成せず、バージョン 7 を必須とする。接続は四つまでとし、取得・文実行・ロックの待ち時間を二秒に制限する。公開テーブルに限定した独立の SELECT 専用ロールに依存する。",
            contracts: &["reader-boundary", "reader-permissions"]
        },
        trace::Guarantee {
            id: "deployment-process-config",
            statement: "サービス設定はアカウントと対局のストレージを分離し、公開読み取りプロセスに独立した認証情報を与え、ゲートウェイにはデータベース認証情報を与えない。アカウントは設定の検証、ストレージ接続、プロバイダー情報取得の後に待ち受ける。対局と読み取りの起動処理は、アドレスを解析し、互換ストレージに接続してから要求を受け付ける。",
            contracts: &[
                "process-config",
                "account-environment",
                "game-process",
                "reader-process"
            ]
        }
    ],
    limitations: &[
        "配置構成と読み取り用データベースの分離には、既存の python3 scripts/test-deployment.py による自動検査がある。この対応グラフが関連付けるのは Cargo テストだけであり、その Docker ベースのコマンドを実行したり置き換えたりはしない。",
        "アプリケーションのヘルス確認用エンドポイントはない。Compose の PostgreSQL と Valkey の疎通検査は、アプリケーションの準備完了を示すものではない。",
        "CLI と環境変数の解析、外部 Google サービスの可用性、起動失敗の伝播、六つすべてのアプリケーション実行ファイルのシグナル停止と接続終了は、実行時テストの不足を明示したソース上の契約として残る。",
        "イベントとレーティングのループの実行時所有権・設定は durable-event-dispatch と rating-application でも記述する。プロセスが常に依存先へ接続できると保証する起動テストは、対応付けられていない。",
        "ソースへのリンクとテストの実行成功は、追跡できる根拠を示すものであり、これらの保証の意味上の証明や完全な網羅性を示すものではない。"
    ],
    entrypoints: &[
        trace::EntryPoint {
            name: "対局 DB の初期化とスキーマ検査",
            source: trace::Source {
                path: "crates/game-persistence/src/schema.rs",
                needle: "pub(crate) async fn initialize(db: &DatabaseConnection) -> Result<(), SchemaError> {"
            }
        },
        trace::EntryPoint {
            name: "公開データ読み取り接続",
            source: trace::Source {
                path: "crates/game-record-persistence/src/lib.rs",
                needle: "pub async fn connect(url: &str) -> Result<Self, Error> {"
            }
        },
        trace::EntryPoint {
            name: "ユーザー DB の初期化とスキーマ検査",
            source: trace::Source {
                path: "crates/user-persistence/src/lib.rs",
                needle: "pub async fn connect(url: &str) -> Result<Self, Error> {"
            }
        },
        trace::EntryPoint {
            name: "ユーザーサービス起動",
            source: trace::Source {
                path: "apps/user-server/src/main.rs",
                needle: "async fn main() -> Result<(), Box<dyn std::error::Error>> {"
            }
        },
        trace::EntryPoint {
            name: "公開データサービス起動",
            source: trace::Source {
                path: "apps/player-server/src/main.rs",
                needle: "async fn main() -> Result<(), Box<dyn std::error::Error>> {"
            }
        },
        trace::EntryPoint {
            name: "リアルタイムゲートウェイ起動",
            source: trace::Source {
                path: "apps/realtime-gateway/src/main.rs",
                needle: "async fn main() -> Result<(), Box<dyn std::error::Error>> {"
            }
        }
    ],
    surfaces: &[],
});
