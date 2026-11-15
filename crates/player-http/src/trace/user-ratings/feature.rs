use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "ユーザー別の確定済みレート取得",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "async fn ratings(State(app): State<App>, path: Result<Path<String>, PathRejection>) -> Response {"
    },
    flow: &[
        "HTTP アダプターは ID を検証し、処理枠を取得して UserRatings::get を呼ぶ。",
        "アプリケーション層は型付き ID を RatingRepository に渡す。",
        "PostgreSQL 読み取り側が確定済み状態を読み、型の検証を通した結果を HTTP が JSON に変換する。"
    ],
    guarantees: &[
        trace::Guarantee {
            id: "explicit-pools",
            statement: "読み取りに成功すると両プールを返す。適用済み対局のないプールは rating が null、completed_rated_games が 0 となる。構文上有効な未知のユーザーにも認証なしで応答する。",
            contracts: &[
                "http-pools",
                "application-forwarding",
                "postgres-snapshot",
                "rating-state"
            ]
        },
        trace::Guarantee {
            id: "committed-snapshot",
            statement: "読み取りに成功するとプール別の確定済みレートを返す。アカウントの存在や未処理対局の適用完了は保証しない。",
            contracts: &["http-pools", "application-forwarding", "postgres-snapshot"]
        },
        trace::Guarantee {
            id: "invalid-user",
            statement: "不正なユーザー ID は保存層を呼ぶ前に 400 で拒否する。",
            contracts: &["http-input", "user-id"]
        },
        trace::Guarantee {
            id: "unavailable",
            statement: "保存層の失敗や HTTP 処理枠の不足は 503 と error: unavailable を返す。",
            contracts: &["http-unavailable"]
        },
        trace::Guarantee {
            id: "no-store",
            statement: "ハンドラーが処理したレスポンスに Cache-Control: no-store を付ける。",
            contracts: &["http-cache"]
        },
        trace::Guarantee {
            id: "openapi-shape",
            statement: "OpenAPI は認証不要のレート取得、200・400・503 の応答、両プールと必須かつ null 許容の rating を記述する。",
            contracts: &["http-openapi"]
        }
    ],
    limitations: &[
        "実 PostgreSQL から HTTP レスポンスまでを通すレート取得の end-to-end テストはない。",
        "リンクの成立とテストの成功は、宣言した保証の意味や網羅性の証明ではない。",
        "保証や契約そのものを削除した変更、孤立を残さない関連の削除、根拠の過大評価はレビューで確認する。",
        "この機能はレート取得を扱う。レート計算、ワーカー、書き込み権限は機能一覧の別の対応表で扱う。"
    ],
    entrypoints: &[],
    surfaces: &[trace::Surface::OpenApi {
        package: "player-http",
        example: "player-openapi",
        operation: "GET /users/{user_id}/ratings"
    }],
});
