use crate::trace_support as trace;
trace::feature!(trace::Feature {
    id: "user-ratings",
    title: "ユーザー別の確定済みレート取得",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "async fn ratings(State(app): State<App>, path: Result<Path<String>, PathRejection>) -> Response {"
    },
    flow: &["フロントエンドの操作を受付境界で検証し、直接の依存先の契約を利用して応答する。"],
    guarantees: &[
        trace::Guarantee {
            id: "explicit-pools",
            statement: "読み取りに成功すると両プールを返す。適用済み対局のないプールは rating が null、completed_rated_games が 0 となる。構文上有効な未知のユーザーにも認証なしで応答する。",
            roots: &["player-http/frontend-get-users-user-id-ratings"]
        },
        trace::Guarantee {
            id: "committed-snapshot",
            statement: "読み取りに成功するとプール別の確定済みレートを返す。アカウントの存在や未処理対局の適用完了は保証しない。",
            roots: &["player-http/frontend-get-users-user-id-ratings"]
        },
        trace::Guarantee {
            id: "invalid-user",
            statement: "不正なユーザー ID は保存層を呼ぶ前に 400 で拒否する。",
            roots: &["player-http/frontend-get-users-user-id-ratings"]
        },
        trace::Guarantee {
            id: "unavailable",
            statement: "保存層の失敗や HTTP 処理枠の不足は 503 と error: unavailable を返す。",
            roots: &["player-http/frontend-get-users-user-id-ratings"]
        },
        trace::Guarantee {
            id: "no-store",
            statement: "ハンドラーが処理したレスポンスに Cache-Control: no-store を付ける。",
            roots: &["player-http/frontend-get-users-user-id-ratings"]
        },
        trace::Guarantee {
            id: "openapi-shape",
            statement: "OpenAPI は認証不要のレート取得、200・400・503 の応答、両プールと必須かつ null 許容の rating を記述する。",
            roots: &["player-http/frontend-get-users-user-id-ratings"]
        }
    ],
    limitations: &[
        "下位の契約や内部テストの成功だけでは、この利用者向け保証の完全性を示さない。",
        "実 PostgreSQL から HTTP レスポンスまでを通すレート取得の end-to-end テストはない。",
        "リンクの成立とテストの成功は、宣言した保証の意味や網羅性の証明ではない。",
        "保証や契約そのものを削除した変更、孤立を残さない関連の削除、根拠の過大評価はレビューで確認する。",
        "この機能はレート取得を扱う。レート計算、ワーカー、書き込み権限は機能一覧の別の対応表で扱う。"
    ]
});
