use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "対局操作の再試行・永続化・復旧",
    source: trace::Source {
        path: "crates/game-application/src/actor.rs",
        needle: "async fn apply(&mut self, pending: MutationRequest) -> Result<Acceptance, ApplicationError> {"
    },
    flow: &[
        "対局ごとのアクターは永続履歴を更新し、参加者への所属、受理済みリクエストの完全一致による再試行、予想シーケンスの順に確認してから、複製した合法な候補局面をステージする。",
        "StagedGameChange は受理結果と正規のイベント列を固定する。commit_change はバージョンを比較し、メタデータ、一件の受理記録、そのイベントを原子的に永続化する。",
        "成否不明または競合したコミットの後、アクターは永続受理記録を再読み込みする。一致する記録があれば元の受理結果を返し、なければ不確かなローカル状態を使わせず、競合または利用不能を返す。",
        "キュー追加後に呼び出し元が取り消しても、アクターのトランザクションは続行する。再起動時には受理記録を復元し、最後のサービス送信元を破棄すると、残るキュー内の操作を処理して終了する。"
    ],
    guarantees: &[
        trace::Guarantee {
            id: "exact-retry",
            statement: "対局ごとのリクエスト識別子は、受理時にだけ確保する。参加者確認後、型付き操作・ユーザー・元の予想シーケンスが一致する再試行は、再起動後も古いシーケンスや終局の確認より先に元の受理結果を返す。受理済みの内容を変更すると競合する。",
            contracts: &["replay", "actor-apply"]
        },
        trace::Guarantee {
            id: "sequence",
            statement: "新たに受理した操作ごとに、シーケンスを i64::MAX まで一度ずつ進める。対局ごとのアクター処理とリポジトリの CAS が、競合するサービスインスタンスを直列化し、競合に負けた側の状態を更新する。",
            contracts: &["actor-apply", "durable-commit", "sequence-cap"]
        },
        trace::Guarantee {
            id: "atomic-events",
            statement: "候補局面はコミット確認後にだけ公開する。メタデータ・受理記録・渡された正規イベントを一緒にコミットする。着手は MoveCommitted を生成し、詰みでは同一シーケンスに GameFinished を追加し、投了は GameFinished を生成する。再試行・拒否・復元では新しいイベントを生成しない。",
            contracts: &["actor-apply", "durable-commit", "canonical-events"]
        },
        trace::Guarantee {
            id: "uncertain-recovery",
            statement: "永続コミットの後にエラーが返る場合がある。アクターは一致する保存済み受理記録と照合するか、後の永続状態の再読み込みが成功するまで自身の状態を無効化する。読み込み失敗時にキャッシュを使って変更を続けることはない。",
            contracts: &["actor-apply", "reconciliation"]
        },
        trace::Guarantee {
            id: "cancellation",
            statement: "対局ごとの 64 件のメールボックスが、キューに入る操作数を制限する。追加後の取消は実行を取り消さず、追加前の取消では実行しない。送信元が閉じた後も、キュー内の操作を処理して終了する。",
            contracts: &["mailbox"]
        }
    ],
    limitations: &[
        "代表的な障害・並行処理テストの成功は、すべてのクラッシュの順序や永続基盤の保証を証明しない。",
        "対局操作の対応関係は、正規イベントの生成と原子的なキュー追加を扱う。配信・購読者への到達・レーティング適用は、それぞれ別の機能の記録で扱う。",
        "アクターやメールボックスの公平性、既知の対局を保持する登録簿の総メモリー量、待機中の呼び出し元数の上限は保証しない。",
        "リンクとテスト成功だけでは意味上の完全性は成立しない。保証、根拠の強さ、分割の変更にはレビューが必要となる。"
    ],
    entrypoints: &[trace::EntryPoint {
        name: "対局アクターと操作の復旧",
        source: trace::Source {
            path: "crates/game-application/src/actor.rs",
            needle: "struct Actor {"
        }
    }],
    surfaces: &[],
});
