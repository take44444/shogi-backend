use crate::trace_support as trace;
trace::feature!(trace::Feature {
    id: "account-session",
    title: "ブラウザーセッションの認証と失効",
    source: trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "async fn logout(State(app): State<App>, headers: HeaderMap) -> Result<Response, HttpError> {"
    },
    flow: &["フロントエンドの操作を受付境界で検証し、直接の依存先の契約を利用して応答する。"],
    guarantees: &[
        trace::Guarantee {
            id: "opaque-authentication",
            statement: "GET /me は一意な 64 文字の hex Cookie を要求し、そのハッシュに一致する有効期限内のセッションだけを認証する。",
            roots: &["user-http/frontend-get-me"]
        },
        trace::Guarantee {
            id: "durable-revocation",
            statement: "同一 Origin からのログアウトは指定セッションを失効させる。失効は store の再接続後も維持され、別セッションは引き続き使える。",
            roots: &["user-http/frontend-post-auth-logout"]
        },
        trace::Guarantee {
            id: "browser-cookie-policy",
            statement: "発行 Cookie は Path=/・HttpOnly・SameSite=Lax を使い、本番 HTTPS ではホスト専用 __Host- 名と Secure を使う。",
            roots: &["user-http/frontend-post-auth-logout"]
        },
        trace::Guarantee {
            id: "consumer-revalidation",
            statement: "realtime の認証は /me が返す本人識別を使い、失効または期限切れを再確認した接続ではその後のコマンドや通知を継続しない。",
            roots: &["realtime-gateway/frontend-ws-upgrade"]
        }
    ],
    limitations: &[
        "下位の契約や内部テストの成功だけでは、この利用者向け保証の完全性を示さない。",
        "一般的なブラウザーの Cookie jar、SameSite や Secure のブラウザーによる実際の強制を試すテストではない。",
        "既存セッションを送ったログインでのローテーションの検証不足は account-google-login に記録する。セッションの固定 TTL 設定と startup は別の機能記録が扱う。",
        "リンクの成立とテストの成功は、記述した保証の意味や網羅性の証明ではない。保証・契約・関連の削除や根拠の過大評価はレビューで確認する。"
    ]
});
