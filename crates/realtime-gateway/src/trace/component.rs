use crate::trace_support as trace;
trace::component!(trace::Component {
    description: "realtime-gateway コンポーネント",
    notes: &[
        "account-session/realtime-authentication。AuthClient::authenticate はセッション Cookie を user HTTP の /me に送り、200 の JSON にある型付き UserId を本人識別として使う。401 は認証失敗、他の異常は unavailable として扱う。 未検証範囲。user HTTP のリダイレクト、非 200/401、壊れた・大きすぎる JSON、通信失敗の分岐をこの記録のテストは直接確認していない。",
        "account-session/realtime-recheck。AuthClient::recheck は保存した Cookie で再認証し、接続開始時の UserId と同じであることを要求する。 未検証範囲。別 UserId を返す応答と idle 接続の定期失効確認は直接確認しない。再確認は既存のコマンド・通知処理時の証拠である。",
        "matchmaking-tickets/browser-owner。ブラウザーハンドラーは型付きの参加処理に入る前に、セッション認証から所有者を導出し、Host の完全一致、変更時に必須の Origin、任意の same-site メタデータを確認する。 未検証範囲。すべての不正 UUID、DELETE 本文の拒否、HTTP 許可枠の飽和、認証や接続先のタイムアウト分岐を網羅する専用の検証はない。",
        "matchmaking-tickets/browser-response。チケット応答はサーバー状態を保持し、新規作成・再試行・拒否・組合せ済みチケットの取消を区別する。ルーターのミドルウェアが no-store を追加する。 未検証範囲。no-store は代表的な成功と Origin の失敗で確認するが、全応答区分は扱わない。DTO のすべての破損と上流エラーの変換には、専用の検証が不足している。",
        "matchmaking-tickets/browser-schema。生成する組合せ用 OpenAPI は、所有者に限定した五つの操作と Cookie 認証の選択肢を含む。",
        "public-api-documents/gateway-schema。ゲートウェイは、注釈を付けたマッチングのパスと二つの Cookie セキュリティ方式を、オフライン OpenAPI 取得関数から出力する。 未検証範囲。このテストはオフラインの OpenAPI を検査し、実際の HTTP レスポンスの全フィールドとの一致は検証しない。",
        "realtime-game-feed/bootstrap-contract。run_connectionは閲覧者用スナップショットの取得前に購読と通知転送を開始する。スナップショットを検証して最初にキューへ入れ、その連番でカーソルを初期化し、蓄積した通知をカーソルに通す。 未検証範囲。稼働中のブローカーの切断・再接続を通して、購読喪失の全経路を確認する端から端までの検証はない。再接続では永続イベントログを再送しない。",
        "realtime-game-feed/cursor。ConnectionStateは連続して整合するコミット済みの内容だけを受け入れ、完全に同じ重複と古い境界イベントを破棄し、同じ連番の詰みの着手と終局の一致を確認する。 未検証範囲。実際のWebSocketで詰みの組を検証するテストはない。カーソルの直接テストは人工的なコミット済み通知を使う。接続終了用の補助処理は、resync_requiredの後の切断と即時切断のどちらも許可する。",
        "realtime-game-feed/event-frame。event_jsonは、対局・連番・種別と着手・結果のフィールドを持つ安定したイベント識別情報を出力し、内部のユーザーIDとリクエストIDを含めない。 未検証範囲。event_idの16進エンコードや、すべての対局識別子に対する一意性を証明する直接の検証はない。",
        "realtime-game-feed/snapshot-frame。snapshot_jsonは閲覧者用スナップショットを、文字列の連番、所有権を持つ盤面・持ち駒、手番・王手・結果・合法手、固定された参加者の割り当てに変換する。上流の不正な列挙値・連番は拒否する。 未検証範囲。選択したWebSocketの検証では、盤面・持ち駒の全フィールドや、上流の不正な値を変換する全分岐を比較しない。スナップショットを構成する値全体はgame-snapshots-accessで扱う。",
        "realtime-game-feed/subscription-pump。通知の転送処理は、上限付きキューを通して購読結果を渡し、障害やキューの枯渇で停止する。run_connectionは通知の枯渇・欠落を接続終了の条件として扱う。 未検証範囲。選択した検証には、ゲートウェイの通知転送キューを直接満杯にするものはない。遅いブラウザーによる送信待ちの蓄積と書き込みのタイムアウトにも専用の検証はない。",
        "realtime-session-commands-limits/browser-headers。browser_headersはOriginとHostがそれぞれ1件で完全一致することを求める。fetch-siteの欠落またはsame-originだけを許可し、対応するshogi.realtime.v1の提示をちょうど1件検出する。 未検証範囲。選択したテストでは、Host、サブプロトコルの重複、ヘッダーの正規化のすべての組み合わせを明示的には検証しない。",
        "realtime-session-commands-limits/command-input。読み取り処理は、フレーム・頻度の上限、厳密なコマンド解析、同時実行コマンド枠1件、上限付きキューを適用する。解析処理は、未知のフィールドと、正規形でない10進連番・USIを拒否する。 未検証範囲。不正な10進文字列、範囲外の数値、未知の全フィールドを列挙する解析テストはない。処理中・同時実行枠、コマンドキュー・出力の上限超過を個別に検証するテストもない。",
        "realtime-session-commands-limits/command-response。command_replyは受理結果とgRPCのコードを、accepted、invalid_input、not_participant、request_conflict、stale_seq・actual_seq、rejected、outcome_unknownへ対応付ける。上流のアクセス不可・破損・内部エラーでは接続を閉じる。 未検証範囲。選択したゲートウェイのテストには、stale_seqのメタデータ、request_conflict、rejected、outcome_unknown、上流の全ステータス・不正な結果から接続終了への対応を直接検証するものはない。",
        "realtime-session-commands-limits/live-checks。command_workerは、session.userを使って上流のコマンドを呼ぶ前に、セッションと現在の対局へのアクセスを再確認する。 未検証範囲。応答前の再確認に関する競合を独立して検証するテストや、上流の変更処理の期限と結果を確認するテストはない。",
        "realtime-session-commands-limits/outgoing。書き込み処理は各フレームの前にセッション・アクセスを再確認し、送信・切断の待機時間を制限する。初期化時は、設定されたバイト数を超えるスナップショットを拒否し、上限付きの出力・通知チャネルを使う。 未検証範囲。遅いブラウザーによる送信待ちの蓄積、出力書き込みの期限切れ、通知キューの枯渇、スナップショットの上流呼び出しのタイムアウトを検証するテストはない。スナップショットの上限は、この根拠が処理を通すrun_connectionで適用する。",
        "realtime-session-commands-limits/session-auth。AuthClientは正しい形式のCookieを1件だけ受け入れ、応答サイズと所要時間を制限してユーザーサービスを呼ぶ。再確認では、返されたユーザーがセッションのユーザーと同じであることを求める。 未検証範囲。認証サービスの停止・リダイレクト・過大なプロフィール、返されたユーザーの不一致、Cookieのエンコードの全分岐を扱う専用の検証はない。",
        "realtime-session-commands-limits/shutdown-contract。shutdownは持続する停止通知を保存する。稼働中の接続はその通知を監視し、アップグレードは新たな処理を拒否する。wait_closedは接続全体の枠が解放されるまでの待機時間を制限する。 未検証範囲。テストはルーター・ライブラリーの停止処理を通す。実行ファイルへのCtrl-C、設定の検証、10秒の終了待機期限は確認しない。wait_closedの待機期限切れも個別には検証しない。",
        "realtime-session-commands-limits/upgrade-permits。アップグレードでは認証・アクセス確認を行い、上流の呼び出しの前後で停止状態を確認する。接続全体とユーザーごとの枠を取得し、フレーム・メッセージの上限を設定する。切断時のDropで枠を解放する。 未検証範囲。上流とのハンドシェイク中の停止を扱う専用の競合検証はない。接続枠のテストは、処理中のハンドシェイクの全状態を網羅しない。",
        "realtime-game-feed/bootstrap。接続は閲覧者用スナップショットを確定する前に購読を開始する。先にスナップショットを送り、その後のコミット済みイベントを配信し、購読開始とスナップショット取得の競合による取りこぼしを防ぐ。",
        "realtime-game-feed/committed-feed。参加者と認証済みの公開対局の観戦者は、コミット済みの着手と終局結果を受信する。イベントJSONは対局・連番・操作・結果を公開し、内部のuser_id・request_idを含めない。",
        "realtime-game-feed/cursor-consistency。完全に同じ重複イベントとスナップショット境界以前のイベントを無視する。連番の欠落や同じ連番で競合する内容を検出すると、再接続が必要となる。対応する詰みの着手と終局イベントは同じ連番を使い、順番に配信する。",
        "realtime-game-feed/reconnect。再接続では最新のスナップショットを取得し、2度目の着手・イベントを作らずに元のコマンド受理結果を再取得できる。ブラウザー向けの永続的なイベント再送は提供しない。",
        "realtime-session-commands-limits/handshake。接続のアップグレードには、OriginとHostの完全一致、指定される場合は同一オリジンのFetchメタデータ、対応するサブプロトコルの提示がちょうど1件、有効なセッションCookieがちょうど1件必要となる。アップグレード前にセッション認証と対局へのアクセスを確認する。",
        "realtime-session-commands-limits/commands。厳密に定義された着手・投了コマンドだけを受け入れ、呼び出し元によるユーザーや先後の指定を受け入れない。expected_seqはi64::MAXまでの正規形の10進文字列とし、ゲートウェイが認証済みの識別情報を補う。受理・拒否には安定した応答コードを使い、結果が不確かな場合はoutcome_unknownとする。",
        "realtime-session-commands-limits/live-authorization。コマンドの送信前と各フレームの送信前に、セッションと対局へのアクセスを再確認する。ログアウト・期限切れ・公開範囲の変更でアクセスを失った場合は、以後の配信前に接続を閉じる。",
        "realtime-session-commands-limits/resource-limits。接続全体・ユーザーごとの接続数、入力フレーム・メッセージのバイト数と頻度、同時実行コマンド1件、上限付きの通知・出力キュー、スナップショットのサイズ、上流処理・出力の期限で接続の処理量を制限する。",
        "realtime-session-commands-limits/shutdown。ゲートウェイの停止は、新たなアップグレードを拒否して既存接続を閉じる。最初の接続前に停止した場合も同様とする。切断後は接続枠を解放する。"
    ],
    external: &[
        trace::External {
            id: "frontend",
            title: "フロントエンド"
        },
        trace::External {
            id: "trusted-service-client",
            title: "信頼済みサービス API の呼出元"
        },
        trace::External {
            id: "postgres",
            title: "PostgreSQL"
        },
        trace::External {
            id: "valkey",
            title: "Valkey"
        },
        trace::External {
            id: "google",
            title: "Google OpenID Connect"
        }
    ],
    operations: &[
        trace::Operation {
            contract: "realtime-gateway/frontend-get-matchmaking-me",
            audience: trace::Audience::Frontend,
            surface: trace::Surface::OpenApi {
                example: "matchmaking-openapi",
                operation: "GET /matchmaking/me",
                package: "realtime-gateway"
            },
            reason: "フロントエンドへ直接公開する HTTP 操作。"
        },
        trace::Operation {
            contract: "realtime-gateway/frontend-put-matchmaking-tickets-ticket-id",
            audience: trace::Audience::Frontend,
            surface: trace::Surface::OpenApi {
                example: "matchmaking-openapi",
                operation: "PUT /matchmaking/tickets/{ticket_id}",
                package: "realtime-gateway"
            },
            reason: "フロントエンドへ直接公開する HTTP 操作。"
        },
        trace::Operation {
            contract: "realtime-gateway/frontend-delete-matchmaking-tickets-ticket-id",
            audience: trace::Audience::Frontend,
            surface: trace::Surface::OpenApi {
                example: "matchmaking-openapi",
                operation: "DELETE /matchmaking/tickets/{ticket_id}",
                package: "realtime-gateway"
            },
            reason: "フロントエンドへ直接公開する HTTP 操作。"
        },
        trace::Operation {
            contract: "realtime-gateway/frontend-post-matchmaking-tickets-ticket-id-heartbeat",
            audience: trace::Audience::Frontend,
            surface: trace::Surface::OpenApi {
                example: "matchmaking-openapi",
                operation: "POST /matchmaking/tickets/{ticket_id}/heartbeat",
                package: "realtime-gateway"
            },
            reason: "フロントエンドへ直接公開する HTTP 操作。"
        },
        trace::Operation {
            contract: "realtime-gateway/frontend-post-matchmaking-tickets-ticket-id-ready",
            audience: trace::Audience::Frontend,
            surface: trace::Surface::OpenApi {
                example: "matchmaking-openapi",
                operation: "POST /matchmaking/tickets/{ticket_id}/ready",
                package: "realtime-gateway"
            },
            reason: "フロントエンドへ直接公開する HTTP 操作。"
        },
        trace::Operation {
            contract: "realtime-gateway/frontend-ws-upgrade",
            audience: trace::Audience::Frontend,
            surface: trace::Surface::WebSocket {
                command: "upgrade",
                path: "/ws/games/{game_id}",
                source: trace::Source {
                    path: "crates/realtime-gateway/src/lib.rs",
                    needle: ".route(\"/ws/games/{game_id}\", get(upgrade))"
                }
            },
            reason: "ブラウザー向けの WebSocket 接続とコマンド。"
        },
        trace::Operation {
            contract: "realtime-gateway/frontend-ws-move",
            audience: trace::Audience::Frontend,
            surface: trace::Surface::WebSocket {
                command: "move",
                path: "/ws/games/{game_id}",
                source: trace::Source {
                    path: "crates/realtime-gateway/src/game.rs",
                    needle: "enum WireCommand {"
                }
            },
            reason: "ブラウザー向けの WebSocket 接続とコマンド。"
        },
        trace::Operation {
            contract: "realtime-gateway/frontend-ws-resign",
            audience: trace::Audience::Frontend,
            surface: trace::Surface::WebSocket {
                command: "resign",
                path: "/ws/games/{game_id}",
                source: trace::Source {
                    path: "crates/realtime-gateway/src/game.rs",
                    needle: "enum WireCommand {"
                }
            },
            reason: "ブラウザー向けの WebSocket 接続とコマンド。"
        }
    ],
    entrypoints: &[
        trace::EntryPoint {
            name: "オフラインのマッチメイキング API 仕様",
            source: trace::Source {
                path: "crates/realtime-gateway/examples/matchmaking-openapi.rs",
                needle: "fn main()"
            }
        },
        trace::EntryPoint {
            name: "スナップショットと通知の接続処理",
            source: trace::Source {
                path: "crates/realtime-gateway/src/lib.rs",
                needle: "async fn run_connection("
            }
        },
        trace::EntryPoint {
            name: "認証済み WebSocket 接続",
            source: trace::Source {
                path: "crates/realtime-gateway/src/lib.rs",
                needle: ".route(\"/ws/games/{game_id}\", get(upgrade))"
            }
        },
        trace::EntryPoint {
            name: "WebSocket move と resign の入力",
            source: trace::Source {
                path: "crates/realtime-gateway/src/game.rs",
                needle: "enum WireCommand {"
            }
        }
    ]
});
