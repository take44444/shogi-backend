use crate::trace_support as trace;
trace::component!(trace::Component {
    description: "player-http コンポーネント",
    notes: &[
        "public-api-documents/contract-player-schema。公開対局サービスのルートから生成する OpenAPI 3.1 は、パッケージの識別情報、過不足のない HTTP メソッドとパス、クエリーとエラーの制約、公開応答の構造を持つ。",
        "public-api-documents/example-exports。公開対局、ユーザー、マッチングの各サンプルの入口は、ライブラリーの文書を整形済み JSON としてシリアライズする。 未検証範囲。対応付けたテストはサンプルのプロセスを起動せず、その標準出力、生成用シェルスクリプト、公開済みファイルも確認しない。同等のサンプルは crates/user-http/examples/user-openapi.rs と crates/realtime-gateway/examples/matchmaking-openapi.rs にある。",
        "public-api-documents/player-runtime。公開対局サービスの文書ルートは、保存層を読み取らずに player_http::openapi と同じ生成済み OpenAPI を返す。",
        "public-record-detail/http-boundary。詳細取得ハンドラーはパスの ID を検証し、共有の処理許可を使い、リポジトリの失敗を詳細を伏せた no-store 付き HTTP 応答へ変換する。 未検証範囲。実ルーターのテスト用データには非公開の対局を作らない。非公開の場合の動作はリポジトリのテストと共有エラー変換で裏付ける。共有の受け入れ件数と応答バイト数の上限は public-record-search に対応付けている。",
        "public-record-detail/public-dto。DetailDto は内部のリクエスト、受領記録、王手、Outbox のフィールドを含めず、メタデータと順序付きの操作 DTO を出力する。 未検証範囲。この検証は一つの着手と一つの投了を対象とし、すべての着手・駒打ち・終了結果のエンコードを網羅しない。",
        "public-record-search/http-resource-policy。共有アダプターはセマフォの許可を即座に取得する。respond はエラーを変換し、no-store を付け、シリアライズした JSON を 1 MiB 以内に制限する。 未検証範囲。サイズの境界には実際の最大ページではなく文字列を使う。検索・詳細取得・レート取得を混在させて処理枠を使い切るテストは対応付けていない。",
        "public-record-search/query-boundary。RawSearch はフィールドを型付きの値として解析し、リポジトリへアクセスする前に不正な値、未知の値、重複したクエリー値を拒否する。",
        "user-ratings/http-cache。respond は応答の Cache-Control ヘッダーを no-store にする。 未検証範囲。登録した不正 ID テストは 400 の Cache-Control を確認しない。",
        "user-ratings/http-input。レート取得ハンドラーは Path の復号と UserId の検証が成功するまで repository を呼ばない。 未検証範囲。このテストには ID の全長境界や、すべての不正なパスの列挙はない。",
        "user-ratings/http-openapi。公開 OpenAPI にレート取得と、両プール・必須フィールド・null 許容 rating のスキーマを含める。",
        "user-ratings/http-pools。HTTP アダプターは UserRatingSnapshot の両プールを JSON に変換し、未評価の null と 0 を保持する。",
        "user-ratings/http-unavailable。レート取得ハンドラーは処理枠取得の失敗と repository の全失敗を Unavailable に変換し、503 を返す。 未検証範囲。処理枠が尽きたレート取得の応答を確認するテストは登録していない。",
        "public-api-documents/offline-runtime-equality。ユーザーサービスと公開対局サービスは、リポジトリへのアクセスやユーザーの外部認証プロバイダーへの呼び出しを行わず、オフライン生成と同じ内容の /openapi.json を認証なしで公開する。",
        "public-api-documents/player-schema。公開対局サービスの OpenAPI はサービスの識別情報を持ち、ルート、クエリーとエラーの契約、公開応答のスキーマを記述する。",
        "public-api-documents/user-schema。ユーザーサービスの OpenAPI はサービスの識別情報を持ち、ルート、Cookie の選択肢、DTO、エラーのメディア型、リダイレクトのヘッダーを記述する。",
        "public-api-documents/matchmaking-document。ゲートウェイのオフライン文書は、所有者ごとに制限された五つのマッチング操作と、本番用・開発用のセッション Cookie の選択肢を含む。",
        "public-api-documents/document-commands。三つのサンプルコマンドはサービスを起動せず、ライブラリーの OpenAPI を JSON として出力する。",
        "public-record-detail/public-detail。認証なしの詳細取得は、公開された終了済み対局のメタデータと、連続した順序付きの公開操作を返す。操作の受領記録、再試行用の識別子、内部状態は含めない。",
        "public-record-detail/hidden-indistinguishable。存在しない対局、進行中の対局、非公開の対局には not_found を返す。不正な形式の ID は保存層へ到達する前に拒否する。",
        "public-record-detail/complete-bounded-history。履歴は構造を検証した後、最大 4096 操作まで受け入れる。破損時は部分的な履歴を返さず、詳細を伏せた 500 を返す。履歴の上限超過には 422、保存層が利用できない場合は 503 を返す。",
        "public-record-detail/one-view。別の接続が読み取りの間にメタデータとジャーナルの両方を変更しても、同じデータベースの読み取り時点から両方を取得する。",
        "public-record-search/public-filtered-metadata。認証なしの検索は、取得時点で公開されている終了済み対局のメタデータだけを返し、指定されたフィルターを AND で組み合わせる。",
        "public-record-search/bounded-input。検索には、非負で開始より終了が後の UTC マイクロ秒区間を指定する。開始は含み終了は含まず、区間は最大 366 日となる。取得件数は 1〜100 で既定値は 30 とし、型付きフィルターに未知フィールドや重複フィールドを認めない。不正な入力は保存層へ到達する前に 400 を返す。",
        "public-record-search/seek-pages。結果は終了時刻、続いてバイナリー形式の対局 ID の降順に並ぶ。カーソルはフィルターに結び付き、ページの取得件数は変更できる。次のページでも公開状態を再確認する。",
        "public-record-search/resource-response-policy。処理対象の応答は詳細を伏せたエラーと no-store を使う。共有 HTTP アダプターは同時に最大 16 件を受け入れ、シリアライズ後の成功応答が 1 MiB を超える場合は拒否する。",
        "user-ratings/explicit-pools。読み取りに成功すると両プールを返す。適用済み対局のないプールは rating が null、completed_rated_games が 0 となる。構文上有効な未知のユーザーにも認証なしで応答する。",
        "user-ratings/committed-snapshot。読み取りに成功するとプール別の確定済みレートを返す。アカウントの存在や未処理対局の適用完了は保証しない。",
        "user-ratings/invalid-user。不正なユーザー ID は保存層を呼ぶ前に 400 で拒否する。",
        "user-ratings/unavailable。保存層の失敗や HTTP 処理枠の不足は 503 と error: unavailable を返す。",
        "user-ratings/no-store。ハンドラーが処理したレスポンスに Cache-Control: no-store を付ける。",
        "user-ratings/openapi-shape。OpenAPI は認証不要のレート取得、200・400・503 の応答、両プールと必須かつ null 許容の rating を記述する。"
    ],
    external: &[],
    operations: &[
        trace::Operation {
            contract: "player-http/frontend-get-openapi-json",
            audience: trace::Audience::Frontend,
            surface: trace::Surface::OpenApi {
                example: "player-openapi",
                operation: "GET /openapi.json",
                package: "player-http"
            },
            reason: "フロントエンドへ直接公開する HTTP 操作。"
        },
        trace::Operation {
            contract: "player-http/frontend-get-game-records-id",
            audience: trace::Audience::Frontend,
            surface: trace::Surface::OpenApi {
                example: "player-openapi",
                operation: "GET /game-records/{id}",
                package: "player-http"
            },
            reason: "フロントエンドへ直接公開する HTTP 操作。"
        },
        trace::Operation {
            contract: "player-http/frontend-get-game-records",
            audience: trace::Audience::Frontend,
            surface: trace::Surface::OpenApi {
                example: "player-openapi",
                operation: "GET /game-records",
                package: "player-http"
            },
            reason: "フロントエンドへ直接公開する HTTP 操作。"
        },
        trace::Operation {
            contract: "player-http/frontend-get-users-user-id-ratings",
            audience: trace::Audience::Frontend,
            surface: trace::Surface::OpenApi {
                example: "player-openapi",
                operation: "GET /users/{user_id}/ratings",
                package: "player-http"
            },
            reason: "フロントエンドへ直接公開する HTTP 操作。"
        }
    ],
    entrypoints: &[]
});
