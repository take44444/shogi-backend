use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "frontend-get-game-records-id",
    consumer: "frontend",
    provider: "player-http",
    interface: trace::Interface::Http,
    statement: "認証なしの詳細取得は、公開された終了済み対局のメタデータと、連続した順序付きの公開操作を返す。操作の受領記録、再試行用の識別子、内部状態は含めない。 存在しない対局、進行中の対局、非公開の対局には not_found を返す。不正な形式の ID は保存層へ到達する前に拒否する。 履歴は構造を検証した後、最大 4096 操作まで受け入れる。破損時は部分的な履歴を返さず、詳細を伏せた 500 を返す。履歴の上限超過には 422、保存層が利用できない場合は 503 を返す。 別の接続が読み取りの間にメタデータとジャーナルの両方を変更しても、同じデータベースの読み取り時点から両方を取得する。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "async fn detail(State(app): State<App>, path: Result<Path<String>, PathRejection>) -> Response {"
    },
    call: None,
    implementation: Some(trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "async fn detail(State(app): State<App>, path: Result<Path<String>, PathRejection>) -> Response {"
    }),
    binding: None,
    requires: &[trace::Dependency {
        contract: "game-record-application/http-record-detail",
        reason: "提供元はこの直接の依存先を利用して、上位の応答と状態の保証を満たす。"
    }],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
