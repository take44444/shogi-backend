use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "公開 API 仕様の取得とオフライン生成",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "pub fn openapi() -> utoipa::openapi::OpenApi {"
    },
    flow: &[
        "ユーザーサービスと公開対局サービスは、OpenApiRouter への登録からオフライン文書と、認証なしで取得できる /openapi.json の応答を生成する。",
        "ゲートウェイは、所有者ごとに制限されたルートと Cookie の選択肢を含む、マッチング用の独立した OpenAPI 文書をオフラインで出力する。",
        "サンプルの実行ファイルは、これらのライブラリー文書をシリアライズする。文書生成時にサービスは起動しない。"
    ],
    guarantees: &[
        trace::Guarantee {
            id: "offline-runtime-equality",
            statement: "ユーザーサービスと公開対局サービスは、リポジトリへのアクセスやユーザーの外部認証プロバイダーへの呼び出しを行わず、オフライン生成と同じ内容の /openapi.json を認証なしで公開する。",
            contracts: &["player-runtime", "user-runtime"]
        },
        trace::Guarantee {
            id: "player-schema",
            statement: "公開対局サービスの OpenAPI はサービスの識別情報を持ち、ルート、クエリーとエラーの契約、公開応答のスキーマを記述する。",
            contracts: &["contract-player-schema"]
        },
        trace::Guarantee {
            id: "user-schema",
            statement: "ユーザーサービスの OpenAPI はサービスの識別情報を持ち、ルート、Cookie の選択肢、DTO、エラーのメディア型、リダイレクトのヘッダーを記述する。",
            contracts: &["contract-user-schema"]
        },
        trace::Guarantee {
            id: "matchmaking-document",
            statement: "ゲートウェイのオフライン文書は、所有者ごとに制限された五つのマッチング操作と、本番用・開発用のセッション Cookie の選択肢を含む。",
            contracts: &["gateway-schema"]
        },
        trace::Guarantee {
            id: "document-commands",
            statement: "三つのサンプルコマンドはサービスを起動せず、ライブラリーの OpenAPI を JSON として出力する。",
            contracts: &["example-exports"]
        }
    ],
    limitations: &[
        "生成スキーマの検証とプロセス内の応答確認だけでは、文書の公開や実際のサンプルコマンドの標準出力は確認できない。",
        "実行中に文書を返すエンドポイントを持つのはユーザーサービスと公開対局サービスだけである。ゲートウェイの文書はマッチングを対象とし、WebSocket プロトコルはその OpenAPI の対象外となる。",
        "ソースへのリンクとテストの実行成功は、追跡できる根拠を示す。保証の意味の証明や完全な網羅性を示すものではない。"
    ],
    entrypoints: &[trace::EntryPoint {
        name: "オフラインのマッチメイキング API 仕様",
        source: trace::Source {
            path: "crates/realtime-gateway/examples/matchmaking-openapi.rs",
            needle: "fn main()"
        }
    }],
    surfaces: &[
        trace::Surface::OpenApi {
            package: "user-http",
            example: "user-openapi",
            operation: "GET /openapi.json"
        },
        trace::Surface::OpenApi {
            package: "player-http",
            example: "player-openapi",
            operation: "GET /openapi.json"
        }
    ],
});
