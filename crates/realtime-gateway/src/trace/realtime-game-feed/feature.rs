use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "対局の初期局面と確定イベントのリアルタイム配信",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "async fn run_connection("
    },
    flow: &[
        "認証済みのアップグレードとアクセス確認の後、run_connectionは閲覧者用スナップショットを要求する前にValkeyの購読を確立し、到着したコミット済み通知をバッファーに保持する。",
        "先にスナップショットを出力キューへ入れる。接続ごとのカーソルは境界以前のイベントと重複を無視し、同じ連番の詰みの組を含め、整合した連続するコミット済みイベントだけを受け入れる。",
        "書き込み処理は、各フレームを送る前にセッション・対局へのアクセスを再確認する。ブラウザー向けイベントには、内部のコマンドのユーザーID・リクエストIDを含めない。連番の欠落、不正な購読入力、上限超過で接続を終了する。",
        "再接続では最新のスナップショットを取得する。クライアントは元のコマンドの識別情報で再試行して受理結果を回復できるが、ブラウザー向けの永続的なイベント再送は行わない。"
    ],
    guarantees: &[
        trace::Guarantee {
            id: "bootstrap",
            statement: "接続は閲覧者用スナップショットを確定する前に購読を開始する。先にスナップショットを送り、その後のコミット済みイベントを配信し、購読開始とスナップショット取得の競合による取りこぼしを防ぐ。",
            contracts: &["bootstrap-contract", "snapshot-frame"]
        },
        trace::Guarantee {
            id: "committed-feed",
            statement: "参加者と認証済みの公開対局の観戦者は、コミット済みの着手と終局結果を受信する。イベントJSONは対局・連番・操作・結果を公開し、内部のuser_id・request_idを含めない。",
            contracts: &["event-frame", "bootstrap-contract"]
        },
        trace::Guarantee {
            id: "cursor-consistency",
            statement: "完全に同じ重複イベントとスナップショット境界以前のイベントを無視する。連番の欠落や同じ連番で競合する内容を検出すると、再接続が必要となる。対応する詰みの着手と終局イベントは同じ連番を使い、順番に配信する。",
            contracts: &["cursor", "subscription-pump"]
        },
        trace::Guarantee {
            id: "reconnect",
            statement: "再接続では最新のスナップショットを取得し、2度目の着手・イベントを作らずに元のコマンド受理結果を再取得できる。ブラウザー向けの永続的なイベント再送は提供しない。",
            contracts: &["bootstrap-contract", "cursor"]
        }
    ],
    limitations: &[
        "通知は一時的なものとする。ブラウザーの再接続では最新のスナップショットを取得し、受理結果は完全に同じコマンドの再試行で回復する。クライアント向けの永続的なイベント再送は保証しない。",
        "共通の接続終了の検証が許すとおり、終了する接続では相手にresync_requiredが届く前に切断する場合がある。",
        "Outboxの配信と購読プロトコルの内部処理は、別のメッセージング機能の定義で扱う。このグラフでは、通知の消費からブラウザーの動作までを対応付ける。",
        "リンクの存在やテストの成功は、意味上の網羅性を証明しない。保証、根拠の強さ、機能の分割を変更する場合はレビューが必要となる。"
    ],
    entrypoints: &[trace::EntryPoint {
        name: "スナップショットと通知の接続処理",
        source: trace::Source {
            path: "crates/realtime-gateway/src/lib.rs",
            needle: "async fn run_connection("
        }
    }],
    surfaces: &[],
});
