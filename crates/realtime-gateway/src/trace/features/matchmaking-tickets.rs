use crate::trace_support as trace;
trace::feature!(trace::Feature {
    id: "matchmaking-tickets",
    title: "マッチメイキングの参加・状態取得・継続・準備確認・キャンセル",
    source: trace::Source {
        path: "crates/realtime-gateway/src/matchmaking.rs",
        needle: "pub(crate) fn router() -> Router<Arc<Gateway>> {"
    },
    flow: &["フロントエンドの操作を受付境界で検証し、直接の依存先の契約を利用して応答する。"],
    guarantees: &[
        trace::Guarantee {
            id: "owner-admission",
            statement: "所有者は対応するどちらのレーティングプールにも参加を要求できるが、プール全体で待機中・準備待ち・対局中の参加は一件に限られる。サーバーは参加時のレーティングを固定し、UUID による再試行の識別を保持する。",
            roots: &[
                "realtime-gateway/frontend-get-matchmaking-me",
                "realtime-gateway/frontend-put-matchmaking-tickets-ticket-id",
                "realtime-gateway/frontend-delete-matchmaking-tickets-ticket-id",
                "realtime-gateway/frontend-post-matchmaking-tickets-ticket-id-heartbeat",
                "realtime-gateway/frontend-post-matchmaking-tickets-ticket-id-ready"
            ]
        },
        trace::Guarantee {
            id: "durable-history",
            statement: "現在参加の取得と、ハートビートや元の操作の再試行による保存済み結果の取得は所有者に限定する。同一の再試行は保存済み結果を保持し、プールの変更は競合する。参加拒否と参加前の取消は永続化される。",
            roots: &[
                "realtime-gateway/frontend-get-matchmaking-me",
                "realtime-gateway/frontend-put-matchmaking-tickets-ticket-id",
                "realtime-gateway/frontend-delete-matchmaking-tickets-ticket-id",
                "realtime-gateway/frontend-post-matchmaking-tickets-ticket-id-heartbeat",
                "realtime-gateway/frontend-post-matchmaking-tickets-ticket-id-ready"
            ]
        },
        trace::Guarantee {
            id: "waiting-lifecycle",
            statement: "待機は、参加受け入れ時またはシーケンスが厳密に大きい最新の正のハートビートから 30 秒続く。同じか古いハートビートでは延長も復活もしない。取消は待機を終了させるが、組合せ済みの対局は取り消せない。",
            roots: &[
                "realtime-gateway/frontend-get-matchmaking-me",
                "realtime-gateway/frontend-put-matchmaking-tickets-ticket-id",
                "realtime-gateway/frontend-delete-matchmaking-tickets-ticket-id",
                "realtime-gateway/frontend-post-matchmaking-tickets-ticket-id-heartbeat",
                "realtime-gateway/frontend-post-matchmaking-tickets-ticket-id-ready"
            ]
        },
        trace::Guarantee {
            id: "browser-contract",
            statement: "ブラウザーの五つの組合せ操作は有効なセッションを必須とし、Host と変更時の Origin・same-site を確認する。呼び出し元の user_id を拒否し、安定したチケット・エラー JSON を返して no-store を適用する。OpenAPI は五つすべての操作を記述する。",
            roots: &[
                "realtime-gateway/frontend-get-matchmaking-me",
                "realtime-gateway/frontend-put-matchmaking-tickets-ticket-id",
                "realtime-gateway/frontend-delete-matchmaking-tickets-ticket-id",
                "realtime-gateway/frontend-post-matchmaking-tickets-ticket-id-heartbeat",
                "realtime-gateway/frontend-post-matchmaking-tickets-ticket-id-ready"
            ]
        },
        trace::Guarantee {
            id: "ready-before-play",
            statement: "組合せから 15 秒の共通期限内に双方が準備完了を送った場合だけ対局を作成する。期限ちょうどを含む失効は両者を Voided にし、勝敗・レーティングを作らず再参加を許す。準備確認の再送は期限を延長せず、新しい参加に影響しない。",
            roots: &[
                "realtime-gateway/frontend-get-matchmaking-me",
                "realtime-gateway/frontend-put-matchmaking-tickets-ticket-id",
                "realtime-gateway/frontend-delete-matchmaking-tickets-ticket-id",
                "realtime-gateway/frontend-post-matchmaking-tickets-ticket-id-heartbeat",
                "realtime-gateway/frontend-post-matchmaking-tickets-ticket-id-ready"
            ]
        }
    ],
    limitations: &[
        "下位の契約や内部テストの成功だけでは、この利用者向け保証の完全性を示さない。",
        "gRPC は呼び出し元の識別情報の申告を受け入れ、ユーザーを認証しない。認証済み所有者の導出はブラウザーゲートウェイが担う。",
        "選択したテストは、ハートビートの全順序、不正な全フィールド、no-store の全応答区分、接続先の失敗の全分岐を網羅しない。",
        "リンクとテスト成功だけでは意味上の完全性は成立しない。保証、根拠の強さ、分割の変更にはレビューが必要となる。"
    ]
});
