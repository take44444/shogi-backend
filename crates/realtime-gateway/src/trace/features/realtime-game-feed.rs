use crate::trace_support as trace;
trace::feature!(trace::Feature {
    id: "realtime-game-feed",
    title: "対局の初期局面と確定イベントのリアルタイム配信",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: ".route(\"/ws/games/{game_id}\", get(upgrade))"
    },
    flow: &["フロントエンドの操作を受付境界で検証し、直接の依存先の契約を利用して応答する。"],
    guarantees: &[
        trace::Guarantee {
            id: "bootstrap",
            statement: "接続は閲覧者用スナップショットを確定する前に購読を開始する。先にスナップショットを送り、その後のコミット済みイベントを配信し、購読開始とスナップショット取得の競合による取りこぼしを防ぐ。",
            roots: &["realtime-gateway/frontend-ws-upgrade"]
        },
        trace::Guarantee {
            id: "committed-feed",
            statement: "参加者と認証済みの公開対局の観戦者は、コミット済みの着手と終局結果を受信する。イベントJSONは対局・連番・操作・結果を公開し、内部のuser_id・request_idを含めない。",
            roots: &["realtime-gateway/frontend-ws-upgrade"]
        },
        trace::Guarantee {
            id: "cursor-consistency",
            statement: "完全に同じ重複イベントとスナップショット境界以前のイベントを無視する。連番の欠落や同じ連番で競合する内容を検出すると、再接続が必要となる。対応する詰みの着手と終局イベントは同じ連番を使い、順番に配信する。",
            roots: &["realtime-gateway/frontend-ws-upgrade"]
        },
        trace::Guarantee {
            id: "reconnect",
            statement: "再接続では最新のスナップショットを取得し、2度目の着手・イベントを作らずに元のコマンド受理結果を再取得できる。ブラウザー向けの永続的なイベント再送は提供しない。",
            roots: &["realtime-gateway/frontend-ws-upgrade"]
        }
    ],
    limitations: &[
        "下位の契約や内部テストの成功だけでは、この利用者向け保証の完全性を示さない。",
        "通知は一時的なものとする。ブラウザーの再接続では最新のスナップショットを取得し、受理結果は完全に同じコマンドの再試行で回復する。クライアント向けの永続的なイベント再送は保証しない。",
        "共通の接続終了の検証が許すとおり、終了する接続では相手にresync_requiredが届く前に切断する場合がある。",
        "Outboxの配信と購読プロトコルの内部処理は、別のメッセージング機能の定義で扱う。このグラフでは、通知の消費からブラウザーの動作までを対応付ける。",
        "リンクの存在やテストの成功は、意味上の網羅性を証明しない。保証、根拠の強さ、機能の分割を変更する場合はレビューが必要となる。"
    ]
});
