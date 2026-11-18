use crate::trace_support as trace;
trace::feature!(trace::Feature {
    id: "account-google-login",
    title: "Google によるブラウザーログイン",
    source: trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "async fn start(State(app): State<App>) -> Result<Response, HttpError> {"
    },
    flow: &["フロントエンドの操作を受付境界で検証し、直接の依存先の契約を利用して応答する。"],
    guarantees: &[
        trace::Guarantee {
            id: "browser-bound",
            statement: "ログイン試行はブラウザーに結び付き、有効期限を過ぎた試行や使用済みの試行はコード交換に使えない。",
            roots: &[
                "user-http/frontend-get-auth-google-start",
                "user-http/frontend-get-auth-google-callback"
            ]
        },
        trace::Guarantee {
            id: "pkce",
            statement: "認可要求は openid・profile と state・nonce・S256 PKCE を使い、コード交換は保存した verifier を送る。",
            roots: &[
                "user-http/frontend-get-auth-google-start",
                "user-http/frontend-get-auth-google-callback"
            ]
        },
        trace::Guarantee {
            id: "verified-identity",
            statement: "Google の署名、issuer、audience、nonce、有効期限、authorized party と、存在する場合の access-token hash を検証してから issuer・subject を本人識別に使う。",
            roots: &[
                "user-http/frontend-get-auth-google-start",
                "user-http/frontend-get-auth-google-callback"
            ]
        },
        trace::Guarantee {
            id: "atomic-account-session",
            statement: "同じ issuer・subject の同時初回ログインは一つのアカウントに収束し、新しい本人識別情報とセッションはまとめて保存される。",
            roots: &[
                "user-http/frontend-get-auth-google-start",
                "user-http/frontend-get-auth-google-callback"
            ]
        }
    ],
    limitations: &[
        "下位の契約や内部テストの成功だけでは、この利用者向け保証の完全性を示さない。",
        "実 Google の discovery・認可・コード交換からブラウザー Cookie までを通す end-to-end テストはない。署名検証、ローカル token endpoint、偽 provider の HTTP フローは別々の証拠である。",
        "ログイン成功時の既存セッションローテーションは未確認。registration_profile_update_csrf_logout_and_restart の二回目のログインは既存セッション Cookie を送らないため、その根拠にはできない。",
        "Cookie 属性とセッションの利用・失効は account-session、プロフィールと表示名は account-profile、設定・startup・schema は別の機能記録が扱う。",
        "リンクの成立とテストの成功は、記述した保証の意味や網羅性の証明ではない。保証・契約・関連の削除や根拠の過大評価はレビューで確認する。"
    ]
});
