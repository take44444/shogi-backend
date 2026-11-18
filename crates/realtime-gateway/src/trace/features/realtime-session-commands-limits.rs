use crate::trace_support as trace;
trace::feature!(trace::Feature {
    id: "realtime-session-commands-limits",
    title: "WebSocket の認証・対局操作・資源制限",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: ".route(\"/ws/games/{game_id}\", get(upgrade))"
    },
    flow: &["フロントエンドの操作を受付境界で検証し、直接の依存先の契約を利用して応答する。"],
    guarantees: &[
        trace::Guarantee {
            id: "handshake",
            statement: "接続のアップグレードには、OriginとHostの完全一致、指定される場合は同一オリジンのFetchメタデータ、対応するサブプロトコルの提示がちょうど1件、有効なセッションCookieがちょうど1件必要となる。アップグレード前にセッション認証と対局へのアクセスを確認する。",
            roots: &[
                "realtime-gateway/frontend-ws-upgrade",
                "realtime-gateway/frontend-ws-move",
                "realtime-gateway/frontend-ws-resign"
            ]
        },
        trace::Guarantee {
            id: "commands",
            statement: "厳密に定義された着手・投了コマンドだけを受け入れ、呼び出し元によるユーザーや先後の指定を受け入れない。expected_seqはi64::MAXまでの正規形の10進文字列とし、ゲートウェイが認証済みの識別情報を補う。受理・拒否には安定した応答コードを使い、結果が不確かな場合はoutcome_unknownとする。",
            roots: &[
                "realtime-gateway/frontend-ws-upgrade",
                "realtime-gateway/frontend-ws-move",
                "realtime-gateway/frontend-ws-resign"
            ]
        },
        trace::Guarantee {
            id: "live-authorization",
            statement: "コマンドの送信前と各フレームの送信前に、セッションと対局へのアクセスを再確認する。ログアウト・期限切れ・公開範囲の変更でアクセスを失った場合は、以後の配信前に接続を閉じる。",
            roots: &[
                "realtime-gateway/frontend-ws-upgrade",
                "realtime-gateway/frontend-ws-move",
                "realtime-gateway/frontend-ws-resign"
            ]
        },
        trace::Guarantee {
            id: "resource-limits",
            statement: "接続全体・ユーザーごとの接続数、入力フレーム・メッセージのバイト数と頻度、同時実行コマンド1件、上限付きの通知・出力キュー、スナップショットのサイズ、上流処理・出力の期限で接続の処理量を制限する。",
            roots: &[
                "realtime-gateway/frontend-ws-upgrade",
                "realtime-gateway/frontend-ws-move",
                "realtime-gateway/frontend-ws-resign"
            ]
        },
        trace::Guarantee {
            id: "shutdown",
            statement: "ゲートウェイの停止は、新たなアップグレードを拒否して既存接続を閉じる。最初の接続前に停止した場合も同様とする。切断後は接続枠を解放する。",
            roots: &[
                "realtime-gateway/frontend-ws-upgrade",
                "realtime-gateway/frontend-ws-move",
                "realtime-gateway/frontend-ws-resign"
            ]
        }
    ],
    limitations: &[
        "下位の契約や内部テストの成功だけでは、この利用者向け保証の完全性を示さない。",
        "ゲートウェイがブラウザーの識別情報を認証する。信頼される上流のgRPCサービス自体は、渡された識別情報を受け入れる。セッション・アクセスの確認は、確認を行う各操作での情報開示を制限するものであり、サービス間で認可状態のスナップショットを一括して確定するものではない。",
        "リソースの上限は、設定されたキュー・枠と期限に適用される。選択したテストは、過負荷やタイムアウトの全分岐を証明するものではない。実装済みの接続枠と同時実行コマンド枠に加えて、ハンドシェイク・入力用の独立したセマフォを設けてはいない。",
        "実行ファイルの起動・設定・シグナル処理は、実行時プロセスの機能定義で扱う。これらのライブラリールーターのテストでは確認しない。",
        "リンクの存在やテストの成功は、意味上の網羅性を証明しない。保証、根拠の強さ、機能の分割を変更する場合はレビューが必要となる。"
    ]
});
