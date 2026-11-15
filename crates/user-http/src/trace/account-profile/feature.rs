use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "自分のプロフィール取得と表示名変更",
    source: trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "async fn rename("
    },
    flow: &[
        "GET /me はセッションに対応する Profile を取得し、id・display_name・created_at の JSON に変換する。外部から内部ユーザー ID を指定して読み書きする操作はない。",
        "PATCH /me は JSON 構造と同一 Origin を検証し、DisplayName で前後の空白と Unicode 文字数・制御文字を扱ってから、セッションに対応するアカウントを更新する。",
        "PostgresAccounts は表示名を保存し、次回の同じ issuer・subject のログインでも既存プロフィールを維持する。共有 UserId は各サービス間の内部本人識別を表す。"
    ],
    guarantees: &[
        trace::Guarantee {
            id: "own-profile",
            statement: "認証された自分のプロフィールを id・display_name・created_at として返す。内部 ID は空でなく 128 UTF-8 バイト以内で Unicode 空白を含まない。",
            contracts: &["profile-response", "user-id"]
        },
        trace::Guarantee {
            id: "browser-input",
            statement: "表示名変更には同一 Origin と認証済みセッションが必要で、不正な JSON や未知フィールドは拒否する。",
            contracts: &["http-rename", "profile-update"]
        },
        trace::Guarantee {
            id: "name-boundaries",
            statement: "表示名は前後の空白を除いた後、1〜80 Unicode 文字で制御文字を含まない値だけを受け入れる。",
            contracts: &["display-name", "http-rename"]
        },
        trace::Guarantee {
            id: "durable-profile",
            statement: "変更した表示名は store の再接続と後続の同じ本人識別によるログインで維持される。",
            contracts: &["profile-update", "existing-identity"]
        }
    ],
    limitations: &[
        "表示名境界のドメインテスト、JSON 拒否の HTTP テスト、成功した更新と永続化の統合テストは別々の証拠であり、全入力境界を HTTP から DB まで通していない。",
        "OpenAPI のプロフィール schema は public-api-documents、セッションの本人確認・Cookie と失効は account-session が扱う。",
        "リンクの成立とテストの成功は、記述した保証の意味や網羅性の証明ではない。保証・契約・関連の削除や根拠の過大評価はレビューで確認する。"
    ],
    entrypoints: &[],
    surfaces: &[
        trace::Surface::OpenApi {
            package: "user-http",
            example: "user-openapi",
            operation: "GET /me"
        },
        trace::Surface::OpenApi {
            package: "user-http",
            example: "user-openapi",
            operation: "PATCH /me"
        }
    ],
});
