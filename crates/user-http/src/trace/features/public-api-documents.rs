use crate::trace_support as trace;
trace::feature!(trace::Feature {
    id: "public-api-documents",
    title: "公開 API 仕様の取得とオフライン生成",
    source: trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {"
    },
    flow: &["フロントエンドの操作を受付境界で検証し、直接の依存先の契約を利用して応答する。"],
    guarantees: &[trace::Guarantee {
        id: "observable-1",
        statement: "ユーザーサービスと公開対局サービスの公開 API 仕様を認証なしで取得できる。記載した操作と応答形式が実装の登録内容に対応する。",
        roots: &[
            "user-http/frontend-get-openapi-json",
            "player-http/frontend-get-openapi-json"
        ]
    }],
    limitations: &[
        "下位の契約や内部テストの成功だけでは、この利用者向け保証の完全性を示さない。",
        "生成スキーマの検証とプロセス内の応答確認だけでは、文書の公開や実際のサンプルコマンドの標準出力は確認できない。",
        "実行中に文書を返すエンドポイントを持つのはユーザーサービスと公開対局サービスだけである。ゲートウェイの文書はマッチングを対象とし、WebSocket プロトコルはその OpenAPI の対象外となる。",
        "ソースへのリンクとテストの実行成功は、追跡できる根拠を示す。保証の意味の証明や完全な網羅性を示すものではない。"
    ]
});
