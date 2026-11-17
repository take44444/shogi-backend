use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "レート差に基づく対戦選択と準備確認",
    source: trace::Source {
        path: "crates/game-application/src/matchmaking.rs",
        needle: "pub fn select_pair(tickets: &[Ticket], now: i64) -> Option<(&Ticket, &Ticket)> {"
    },
    flow: &[
        "外部依存のない選択処理は、期限内の待機チケットを抽出し、決定的な順序で基準側を並べ、一つのプール内で双方のレーティング許容幅を確認する。",
        "永続 tick のトランザクションは、ロック待ち後にロック済みチケットとレーティングを再確認し、対局 ID と先後を予約して両チケットを Matched にする。双方の準備確認が期限内に揃うと、対局とレーティング予約枠を作り Playing にする。",
        "game-server の組合せ処理は一秒間隔で tick を呼び、最初のプールを交互に替える。別の参加要求を必要とせず、回数を制限したバッチを実行する。"
    ],
    guarantees: &[
        trace::Guarantee {
            id: "policy",
            statement: "組合せ処理は不適合な先頭を読み飛ばし、最も古い適合する基準側と最もレーティング差の小さい相手を選ぶ。同値の場合は時刻・UUID・所有者で順序を決め、別々のユーザーと同一プールを必須とし、双方の拡大した許容幅を適用する。",
            contracts: &["windows", "selection"]
        },
        trace::Guarantee {
            id: "server-ownership",
            statement: "サーバーが対局と固定された先手・後手の割り当てを決める。組合せの成立と二枚のチケットの遷移は一緒にコミットされ、同時に動く組合せ処理では一つだけが永続的な結果を確定する。",
            contracts: &["durable-tick", "readiness"]
        },
        trace::Guarantee {
            id: "expiry",
            statement: "レーティングのロック待ち中に失効した場合も含め、期限切れチケットは組み合わせられない。",
            contracts: &["selection", "durable-tick"]
        },
        trace::Guarantee {
            id: "ready-before-play",
            statement: "組合せから 30 秒の共通期限内に双方が準備完了を送った場合だけ対局を作成する。期限ちょうどを含む失効は両者を Voided にし、勝敗・レーティングを作らず再参加を許す。準備確認の再送は期限を延長せず、新しい参加に影響しない。",
            contracts: &["readiness", "periodic"]
        },
        trace::Guarantee {
            id: "periodic-progress",
            statement: "実際の game-server は定期的に組合せ処理を行うため、追加の参加がなくても待機中の二人を組み合わせ、期限切れの準備待ちを無効にする。各バッチは開始プールを交互に替え、試行を 32 回に制限し、二つのプールがともに空か失敗した時点で止まる。",
            contracts: &["periodic"]
        }
    ],
    limitations: &[
        "プロセステストが示すのは、一つのプールの事例で定期的に処理が進むことまでである。外部依存のないテストは同値時の優先順位を直接検証するが、本番負荷での公平性や時機は証明しない。",
        "プール名は、時計や持ち時間による裁定を有効にしない。",
        "リンクとテスト成功だけでは意味上の完全性は成立しない。保証、根拠の強さ、分割の変更にはレビューが必要となる。"
    ],
    entrypoints: &[trace::EntryPoint {
        name: "待機チケットの周期的マッチング",
        source: trace::Source {
            path: "apps/game-server/src/main.rs",
            needle: "let matcher = tokio::spawn(async move {"
        }
    }],
    surfaces: &[],
});
