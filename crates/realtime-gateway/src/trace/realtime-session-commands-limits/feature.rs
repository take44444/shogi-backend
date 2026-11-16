use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "WebSocket の認証・対局操作・資源制限",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "async fn upgrade("
    },
    flow: &[
        "アップグレードでは、ブラウザーのOriginとHostの完全一致、指定される場合の同一サイトのメタデータ、shogi.realtime.v1の提示が1件であることを検証する。その後、ユーザーサービスでセッションCookieを確認し、GameServiceで対局へのアクセスを確認する。",
        "読み取り処理は厳密に定義された着手・投了JSONと正規形の10進連番を検証する。同時実行枠1件と上限付きキューでコマンドの処理量を制限する。ゲートウェイが認証済みユーザーを上流へのコマンドに補う。",
        "コマンド処理と書き込み処理は、操作前とフレーム送信前にセッション・アクセスを再確認する。受理結果とエラーを安定したブラウザー向けフレームに変換し、結果が不確かな場合は完全に同じ内容での再試行を必要とする。",
        "接続・ユーザーごとの枠、入力バイト数・頻度、スナップショット・バッファーの上限、上流処理・出力の期限で接続の処理量を制限する。ゲートウェイの停止時はアップグレードを拒否し、稼働中のソケットを閉じる。"
    ],
    guarantees: &[
        trace::Guarantee {
            id: "handshake",
            statement: "接続のアップグレードには、OriginとHostの完全一致、指定される場合は同一オリジンのFetchメタデータ、対応するサブプロトコルの提示がちょうど1件、有効なセッションCookieがちょうど1件必要となる。アップグレード前にセッション認証と対局へのアクセスを確認する。",
            contracts: &["browser-headers", "session-auth", "upgrade-permits"]
        },
        trace::Guarantee {
            id: "commands",
            statement: "厳密に定義された着手・投了コマンドだけを受け入れ、呼び出し元によるユーザーや先後の指定を受け入れない。expected_seqはi64::MAXまでの正規形の10進文字列とし、ゲートウェイが認証済みの識別情報を補う。受理・拒否には安定した応答コードを使い、結果が不確かな場合はoutcome_unknownとする。",
            contracts: &["command-input", "command-response", "live-checks"]
        },
        trace::Guarantee {
            id: "live-authorization",
            statement: "コマンドの送信前と各フレームの送信前に、セッションと対局へのアクセスを再確認する。ログアウト・期限切れ・公開範囲の変更でアクセスを失った場合は、以後の配信前に接続を閉じる。",
            contracts: &["session-auth", "live-checks", "outgoing"]
        },
        trace::Guarantee {
            id: "resource-limits",
            statement: "接続全体・ユーザーごとの接続数、入力フレーム・メッセージのバイト数と頻度、同時実行コマンド1件、上限付きの通知・出力キュー、スナップショットのサイズ、上流処理・出力の期限で接続の処理量を制限する。",
            contracts: &["upgrade-permits", "command-input", "outgoing"]
        },
        trace::Guarantee {
            id: "shutdown",
            statement: "ゲートウェイの停止は、新たなアップグレードを拒否して既存接続を閉じる。最初の接続前に停止した場合も同様とする。切断後は接続枠を解放する。",
            contracts: &["shutdown-contract", "upgrade-permits"]
        }
    ],
    limitations: &[
        "ゲートウェイがブラウザーの識別情報を認証する。信頼される上流のgRPCサービス自体は、渡された識別情報を受け入れる。セッション・アクセスの確認は、確認を行う各操作での情報開示を制限するものであり、サービス間で認可状態のスナップショットを一括して確定するものではない。",
        "リソースの上限は、設定されたキュー・枠と期限に適用される。選択したテストは、過負荷やタイムアウトの全分岐を証明するものではない。実装済みの接続枠と同時実行コマンド枠に加えて、ハンドシェイク・入力用の独立したセマフォを設けてはいない。",
        "実行ファイルの起動・設定・シグナル処理は、実行時プロセスの機能定義で扱う。これらのライブラリールーターのテストでは確認しない。",
        "リンクの存在やテストの成功は、意味上の網羅性を証明しない。保証、根拠の強さ、機能の分割を変更する場合はレビューが必要となる。"
    ],
    entrypoints: &[
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
    ],
    surfaces: &[],
});
