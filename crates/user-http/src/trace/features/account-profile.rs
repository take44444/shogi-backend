use crate::trace_support as trace;
trace::feature!(trace::Feature {
    id: "account-profile",
    title: "自分のプロフィール取得と表示名変更",
    source: trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "async fn me(State(app): State<App>, headers: HeaderMap) -> Result<Json<ProfileBody>, HttpError> {"
    },
    flow: &["フロントエンドの操作を受付境界で検証し、直接の依存先の契約を利用して応答する。"],
    guarantees: &[
        trace::Guarantee {
            id: "own-profile",
            statement: "認証された自分のプロフィールを id・display_name・created_at として返す。内部 ID は空でなく 128 UTF-8 バイト以内で Unicode 空白を含まない。",
            roots: &["user-http/frontend-get-me", "user-http/frontend-patch-me"]
        },
        trace::Guarantee {
            id: "browser-input",
            statement: "表示名変更には同一 Origin と認証済みセッションが必要で、不正な JSON や未知フィールドは拒否する。",
            roots: &["user-http/frontend-get-me", "user-http/frontend-patch-me"]
        },
        trace::Guarantee {
            id: "name-boundaries",
            statement: "表示名は前後の空白を除いた後、1〜80 Unicode 文字で制御文字を含まない値だけを受け入れる。",
            roots: &["user-http/frontend-get-me", "user-http/frontend-patch-me"]
        },
        trace::Guarantee {
            id: "durable-profile",
            statement: "変更した表示名は store の再接続と後続の同じ本人識別によるログインで維持される。",
            roots: &["user-http/frontend-get-me", "user-http/frontend-patch-me"]
        }
    ],
    limitations: &[
        "下位の契約や内部テストの成功だけでは、この利用者向け保証の完全性を示さない。",
        "表示名境界のドメインテスト、JSON 拒否の HTTP テスト、成功した更新と永続化の統合テストは別々の証拠であり、全入力境界を HTTP から DB まで通していない。",
        "OpenAPI のプロフィール schema は public-api-documents、セッションの本人確認・Cookie と失効は account-session が扱う。",
        "リンクの成立とテストの成功は、記述した保証の意味や網羅性の証明ではない。保証・契約・関連の削除や根拠の過大評価はレビューで確認する。"
    ]
});
