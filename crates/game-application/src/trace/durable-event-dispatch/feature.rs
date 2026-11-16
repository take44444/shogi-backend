use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "対局イベントの永続化と順序付き配信",
    source: trace::Source {
        path: "crates/game-application/src/events.rs",
        needle: "pub async fn publish_next(&mut self) -> Result<bool, DispatchError> {"
    },
    flow: &[
        "StagedGameChange は受理記録から正規のイベントを固定し、PostgreSQL のコミット処理は受理記録と Outbox の行を原子的に書き込む。",
        "排他ロックを持つ PostgreSQL セッションが、配信中にトランザクションを保持せず、最も古い未処理行を取得する。",
        "OutboxPublisher はバージョン 1 の JSON を正確な対局チャネルに送り、通信先による受け入れを確認してから、選択した一行だけを処理済みにする。",
        "送信または確認の成否が不明な場合も、同じ識別子で再試行して再配信できる。"
    ],
    guarantees: &[
        trace::Guarantee {
            id: "atomic-facts",
            statement: "受理記録と、固定された正規の Outbox イベントを一緒にコミットする。拒否と同一リクエストの再試行ではイベントを追加しない。",
            contracts: &["canonical-facts", "atomic-outbox"]
        },
        trace::Guarantee {
            id: "ordered-identity",
            statement: "配信はキューの順序と安定した識別子 (game_id, seq, kind) を保持する。送信または処理済み更新の成否不明時には、未処理の仕事を飛ばさず、同じ識別子を重複配信する場合がある。",
            contracts: &["exclusive-queue", "confirmed-dispatch", "wire-format"]
        },
        trace::Guarantee {
            id: "confirmed-acceptance",
            statement: "購読者数 0 を含む非負整数の PUBLISH 応答を受け取った後にだけ、対象の一行を処理済みにする。これは通信先による受け入れの確認であり、購読者への到達やブローカーの永続保存の確認ではない。",
            contracts: &["confirmed-dispatch"]
        },
        trace::Guarantee {
            id: "exclusive-fail-closed",
            statement: "一つの PostgreSQL セッションが配信ロックを所有する。そのセッションを失ったら処理を止め、配信を呼ぶ境界ではデータベースのトランザクションを開いたままにしない。",
            contracts: &["exclusive-queue"]
        },
        trace::Guarantee {
            id: "process-dispatch",
            statement: "配信プロセスはキューと通信先を所有し、未処理イベントを定期的に確認する。配信の失敗または Ctrl-C で停止する。",
            contracts: &["publisher-process"]
        }
    ],
    limitations: &[
        "実データベース、起動済み Valkey、稼働中の購読者を一緒に通る対応テストはない。購読者への到達やブローカーの永続保存は保証しない。",
        "キューロックやアダプターの取消には専用テストがあるが、独立したプロセスの起動・再起動・シグナル処理は未検証事項として残る。",
        "スキーマ検証とデータベースのロール方針は startup-schema-isolation に関連付ける。",
        "ソースへのリンクとテストの実行成功は、追跡できる根拠を示すものであり、これらの保証の意味上の証明や完全な網羅性を示すものではない。"
    ],
    entrypoints: &[
        trace::EntryPoint {
            name: "永続 Outbox の一件配信",
            source: trace::Source {
                path: "crates/game-application/src/events.rs",
                needle: "pub async fn publish_next(&mut self) -> Result<bool, DispatchError> {"
            }
        },
        trace::EntryPoint {
            name: "Outbox 配信プロセス",
            source: trace::Source {
                path: "apps/outbox-publisher/src/main.rs",
                needle: "fn main() -> Result<(), Error> {"
            }
        }
    ],
    surfaces: &[],
});
