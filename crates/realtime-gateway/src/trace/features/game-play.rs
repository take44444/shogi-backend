use crate::trace_support as trace;
trace::feature!(trace::Feature {
    id: "game-play",
    title: "着手・将棋ルール・詰み・投了",
    source: trace::Source {
        path: "crates/realtime-gateway/src/game.rs",
        needle: "enum WireCommand {"
    },
    flow: &["フロントエンドの操作を受付境界で検証し、直接の依存先の契約を利用して応答する。"],
    guarantees: &[
        trace::Guarantee {
            id: "participant-input",
            statement: "割り当てられた参加者だけが変更できる。先後は固定された割り当てから決まり、着手は厳密な USI を使う。不正な通信上の値は対局変更前に失敗する。",
            roots: &[
                "realtime-gateway/frontend-ws-move",
                "realtime-gateway/frontend-ws-resign"
            ]
        },
        trace::Guarantee {
            id: "shogi-rules",
            statement: "合法手処理は、すべての通常駒・成駒の移動、移動を遮る駒、捕獲と持ち駒への生駒化、駒打ち、二歩、行き所のない駒の制限、任意・強制の成り、玉の安全と王手回避、打ち歩詰めの拒否に対応する。",
            roots: &[
                "realtime-gateway/frontend-ws-move",
                "realtime-gateway/frontend-ws-resign"
            ]
        },
        trace::Guarantee {
            id: "terminal",
            statement: "詰みは詰ませた側を勝者として対局を終了する。王手でないときに合法手がなくても敗北とはしない。どちらの側も手番外で投了でき、相手が勝者となる。終局後は新しい着手と投了を拒否し、合法手を返さない。",
            roots: &[
                "realtime-gateway/frontend-ws-move",
                "realtime-gateway/frontend-ws-resign"
            ]
        },
        trace::Guarantee {
            id: "atomic-rejection",
            statement: "ドメインが操作を拒否すると対局全体を保持し、アプリケーションは拒否した操作による局面変更をコミット済みとして公開しない。ルールエンジンの失敗は通常の非合法手と区別する。",
            roots: &[
                "realtime-gateway/frontend-ws-move",
                "realtime-gateway/frontend-ws-resign"
            ]
        },
        trace::Guarantee {
            id: "adjudication-scope",
            statement: "千日手や連続王手の局面でも対局を続行できる。時計、時間切れ、入玉・宣言、千日手の裁定はない。プール名はこれらを有効にしない。",
            roots: &[
                "realtime-gateway/frontend-ws-move",
                "realtime-gateway/frontend-ws-resign"
            ]
        },
        trace::Guarantee {
            id: "exact-retry",
            statement: "同じ利用者・操作内容・元の予想シーケンスの再試行は、再起動後も元の受理結果を返す。受理済みの識別子で内容を変更すると競合する。",
            roots: &[
                "realtime-gateway/frontend-ws-move",
                "realtime-gateway/frontend-ws-resign"
            ]
        }
    ],
    limitations: &[
        "下位の契約や内部テストの成功だけでは、この利用者向け保証の完全性を示さない。",
        "対応するすべてのルール系統には、テスト用局面による直接の根拠がある。選択した実ネットワークテストが扱うのは代表的な合法手順と投了であり、全ルールや詰みに至る全手順ではない。",
        "SFEN からの構築はテスト用局面の準備専用であり、公開された取り込み・編集機能ではない。クライアントが任意に指定できる対局作成 RPC は提供しない。",
        "gRPC 呼び出し元の識別情報は信頼する申告値である。ブラウザー認証となりすましの拒否は realtime-session-commands-limits で追跡する。",
        "リンクとテスト成功だけでは意味上の完全性は成立しない。保証、根拠の強さ、分割の変更にはレビューが必要となる。"
    ]
});
