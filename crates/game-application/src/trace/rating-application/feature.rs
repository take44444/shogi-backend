use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "レート更新ジョブの実行と隔離からの復旧",
    source: trace::Source {
        path: "crates/game-application/src/rating.rs",
        needle: "pub async fn tick(&self, owner: &WorkerId) -> Result<TickOutcome, RatingWorkerError> {"
    },
    flow: &[
        "レーティング対象対局の終局を受理すると、固定された参加時のメタデータと確定結果から永続レーティングジョブを原子的に作成する。ブローカーには依存しない。",
        "RatingWorker は取得世代とリースで保護された取得権を確保し、プールごとに固定された開始状態から計算して、リポジトリに原子的な適用を要求する。",
        "リポジトリは一つのトランザクションで、両者の更新、適用台帳一件の記録、ジョブ完了、その対局の予約枠だけの解放を行う。",
        "一時的な失敗は待機時間を設けて遅延させる。破損または取得回数の枠の使い切りでは、予約枠を持つ対局を明示的な復旧まで隔離する。"
    ],
    guarantees: &[
        trace::Guarantee {
            id: "terminal-durable-input",
            statement: "永続化されたレーティング対象対局の終局だけが、受理記録と Outbox と原子的にレーティングジョブを生成する。計算にはプールごとに固定された開始状態と確定した勝者を使う。",
            contracts: &["terminal-job", "pure-calculation", "worker-reconciliation"]
        },
        trace::Guarantee {
            id: "zero-sum-checked-elo",
            statement: "K32 Elo は未評価の両者を 1500 と扱い、先手の差分を一度だけ丸め、後手にはその符号を反転した差分を適用する。符号付きレーティングと完了対局数の算術を検査する。",
            contracts: &["pure-calculation"]
        },
        trace::Guarantee {
            id: "atomic-once-ledger",
            statement: "有効な現在の取得権に基づく適用は、両者のレーティング更新、台帳一件の挿入、ジョブ完了、その対局自身の予約枠だけの解放を原子的に行う。一致する完了済み処理の再試行は、読み取りだけで結果を照合する。",
            contracts: &["atomic-application", "worker-reconciliation"]
        },
        trace::Guarantee {
            id: "fenced-retry",
            statement: "失効または更新済みの取得権では結果を変更できない。試行回数・リース期限・一時的失敗の待機時間を永続化し、破損や試行枠の使い切りでは予約枠を解放せず隔離する。",
            contracts: &[
                "claims-fences",
                "worker-reconciliation",
                "quarantine-recovery"
            ]
        },
        trace::Guarantee {
            id: "explicit-recovery",
            statement: "明示的な復旧は、入力の修復や固定された開始状態・レーティング・予約枠の変更をせずに、隔離ジョブの取得回数の枠を戻す。ワーカー実行ファイルは、定期実行と retry-quarantined の両モードを提供する。",
            contracts: &["quarantine-recovery", "worker-process"]
        }
    ],
    limitations: &[
        "ワーカーはブローカーメッセージではなく永続ジョブを読む。入力の自動修復や、有限時間内の完了は保証しない。",
        "リポジトリの結合テストでは、独立したワーカー CLI、定期実行、シグナルによるライフサイクルを検証しない。",
        "公開レーティングの読み取りは user-ratings、起動時スキーマと読み取り権限は startup-schema-isolation に関連付ける。",
        "ソースへのリンクとテストの実行成功は、追跡できる根拠を示すものであり、これらの保証の意味上の証明や完全な網羅性を示すものではない。"
    ],
    entrypoints: &[
        trace::EntryPoint {
            name: "レートジョブのポーリングと隔離復旧 CLI",
            source: trace::Source {
                path: "apps/rating-worker-server/src/main.rs",
                needle: "async fn main() -> Result<(), Box<dyn std::error::Error>> {"
            }
        },
        trace::EntryPoint {
            name: "レートジョブの処理",
            source: trace::Source {
                path: "crates/game-application/src/rating.rs",
                needle: "pub async fn tick(&self, owner: &WorkerId) -> Result<TickOutcome, RatingWorkerError> {"
            }
        }
    ],
    surfaces: &[],
});
