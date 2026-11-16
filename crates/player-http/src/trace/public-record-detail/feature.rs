use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "終了済み公開対局の履歴取得",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "async fn detail(State(app): State<App>, path: Result<Path<String>, PathRejection>) -> Response {"
    },
    flow: &[
        "HTTP アダプターは対局 ID を検証し、共有のリクエスト上限の下で GameRecords::detail を呼ぶ。",
        "PostgreSQL の読み取り処理は、一つの反復可能読み取りスナップショットで、公開された終了済み対局のメタデータと、上限内のジャーナル全体を取得する。",
        "型付きメタデータと連続した公開操作を公開 DTO に変換する。破損時は部分的な履歴を返さず、エラーとする。"
    ],
    guarantees: &[
        trace::Guarantee {
            id: "public-detail",
            statement: "認証なしの詳細取得は、公開された終了済み対局のメタデータと、連続した順序付きの公開操作を返す。操作の受領記録、再試行用の識別子、内部状態は含めない。",
            contracts: &[
                "http-boundary",
                "public-dto",
                "metadata-invariants",
                "snapshot-history"
            ]
        },
        trace::Guarantee {
            id: "hidden-indistinguishable",
            statement: "存在しない対局、進行中の対局、非公開の対局には not_found を返す。不正な形式の ID は保存層へ到達する前に拒否する。",
            contracts: &["http-boundary", "snapshot-history"]
        },
        trace::Guarantee {
            id: "complete-bounded-history",
            statement: "履歴は構造を検証した後、最大 4096 操作まで受け入れる。破損時は部分的な履歴を返さず、詳細を伏せた 500 を返す。履歴の上限超過には 422、保存層が利用できない場合は 503 を返す。",
            contracts: &["http-boundary", "metadata-invariants", "snapshot-history"]
        },
        trace::Guarantee {
            id: "one-view",
            statement: "別の接続が読み取りの間にメタデータとジャーナルの両方を変更しても、同じデータベースの読み取り時点から両方を取得する。",
            contracts: &["application-forwarding", "snapshot-history"]
        }
    ],
    limitations: &[
        "対応付けたテストは player-server の TCP リスナーを起動しない。構造上読み取れる履歴が、将棋の合法な着手で構成されていることは保証しない。",
        "共有 HTTP 処理の応答バイト数と受け入れ件数の上限、および読み取りロールとタイムアウトは、public-record-search と startup-schema-isolation に対応付けている。",
        "ソースへのリンクとテストの実行成功は、追跡できる根拠を示す。保証の意味の証明や完全な網羅性を示すものではない。"
    ],
    entrypoints: &[],
    surfaces: &[trace::Surface::OpenApi {
        package: "player-http",
        example: "player-openapi",
        operation: "GET /game-records/{id}"
    }],
});
