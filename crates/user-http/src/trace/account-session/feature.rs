use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "ブラウザーセッションの認証と失効",
    source: trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "async fn logout("
    },
    flow: &[
        "GET /me は一意な不透明セッション Cookie を検証し、SHA-256 を使って PostgreSQL の未失効セッションからプロフィールを取得する。",
        "POST /auth/logout はブラウザーの Origin を検証し、有効な形式の Cookie があれば保存済みセッションを削除して Cookie を消去する。",
        "realtime-gateway は user HTTP の /me で本人識別を解決し、コマンドや通知の処理時に同じセッションの本人識別を再確認する。"
    ],
    guarantees: &[
        trace::Guarantee {
            id: "opaque-authentication",
            statement: "GET /me は一意な 64 文字の hex Cookie を要求し、そのハッシュに一致する有効期限内のセッションだけを認証する。",
            contracts: &["session-cookie-input", "session-storage"]
        },
        trace::Guarantee {
            id: "durable-revocation",
            statement: "同一 Origin からのログアウトは指定セッションを失効させる。失効は store の再接続後も維持され、別セッションは引き続き使える。",
            contracts: &["http-logout", "session-storage"]
        },
        trace::Guarantee {
            id: "browser-cookie-policy",
            statement: "発行 Cookie は Path=/・HttpOnly・SameSite=Lax を使い、本番 HTTPS ではホスト専用 __Host- 名と Secure を使う。",
            contracts: &["cookie-issuance"]
        },
        trace::Guarantee {
            id: "consumer-revalidation",
            statement: "realtime の認証は /me が返す本人識別を使い、失効または期限切れを再確認した接続ではその後のコマンドや通知を継続しない。",
            contracts: &["realtime-authentication", "realtime-recheck"]
        }
    ],
    limitations: &[
        "一般的なブラウザーの Cookie jar、SameSite や Secure のブラウザーによる実際の強制を試すテストではない。",
        "既存セッションを送ったログインでのローテーションの検証不足は account-google-login に記録する。セッションの固定 TTL 設定と startup は別の機能記録が扱う。",
        "リンクの成立とテストの成功は、記述した保証の意味や網羅性の証明ではない。保証・契約・関連の削除や根拠の過大評価はレビューで確認する。"
    ],
    entrypoints: &[],
    surfaces: &[trace::Surface::OpenApi {
        package: "user-http",
        example: "user-openapi",
        operation: "POST /auth/logout"
    }],
});
