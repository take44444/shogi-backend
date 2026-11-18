use crate::trace_support as trace;
trace::component!(trace::Component {
    description: "game-application コンポーネント",
    notes: &[
        "durable-event-dispatch/canonical-facts。StagedGameChange はシーケンスの境界と操作・結果の整合性を検証し、着手に続く終局、または投了だけの終局のイベントを固定する。",
        "durable-event-dispatch/confirmed-dispatch。OutboxPublisher は配信してから処理済みにする。Valkey アダプターは有効な非負整数の PUBLISH 応答だけを受け入れ、取消またはタイムアウトした接続を無効化する。 未検証範囲。通信テストは制御可能な TCP RESP のテスト用実装とテスト用リポジトリを使う。起動済み Valkey や購読者は使わない。",
        "game-command-recovery/actor-apply。変更処理は状態を更新し、予想シーケンスの確認より先に再試行を扱う。複製した候補局面と渡すイベントをステージし、コミット確認後にだけメモリー内の候補を公開する。",
        "game-command-recovery/canonical-events。StagedGameChange はシーケンスの境界と連続性、操作と結果の組み合わせを検証し、通常着手、着手と詰み、投了のイベント順を固定する。",
        "game-command-recovery/mailbox。対局ごとの操作窓口は、操作 64 件の有界チャネルを使う。応答の受信側が破棄されてもアクターは実行を続け、閉じたキュー内の操作を処理して終了する。 未検証範囲。64 件の上限は処理中の操作と送信待ちの呼び出しを除いたキュー内の件数に適用する。既知の対局を保持するアクター登録簿全体の上限や公平性は保証していない。",
        "game-command-recovery/reconciliation。再読み込みは、永続履歴の読み取りと検証の前にローカル状態を消去する。照合に失敗すると、後の再読み込みで復元できるまで利用不能のままとなる。 未検証範囲。障害注入アダプターは代表的な結果経路を制御するが、任意のプロセス実行やクラッシュの割り込み順序は証明しない。",
        "game-command-recovery/replay。再試行処理は最初に参加者への所属を確認し、対局ごとに受理済みの ID に対応する MutationRequest 全体を比較して、元の Acceptance を返す。",
        "game-command-recovery/sequence-cap。next_sequence は i64::MAX から先への更新を拒否する。上限到達後の更新を試す前に、古い予想シーケンスを確認する。 未検証範囲。テストは最大状態を合成する。ルールエンジンが実際に何十億手もの着手を処理できることは証明しない。",
        "game-play/binding。アクターは受理済み記録の検索より先に参加者への所属を確認し、呼び出し元が選んだ先後ではなく、不変の Participants から操作する側を導出する。",
        "game-snapshots-access/access-policy。公開状態または固定された参加者への所属によってアクセスを許可する。CheckViewAccess は履歴を読み込まずメタデータを使う。 未検証範囲。アクセス可否だけを調べる処理が履歴の読み込みを省くことを示す問い合わせ回数の検証はなく、ストレージやアクセス情報の破損分岐もすべては列挙していない。",
        "game-snapshots-access/restore。復元処理は標準初期局面から操作を再現し、受理記録のシーケンス、一意性、参加者の正当性、受理結果、最終メタデータを検証する。",
        "game-snapshots-access/snapshot-refresh。通常のスナップショット要求は、アクターの状態を返す前に永続バージョンを確認する。ストレージ失敗時にキャッシュした局面へ切り替えることはない。",
        "game-snapshots-access/viewer-read。snapshot_for_viewer は、リポジトリの一つのアクセス情報・履歴の読み取り結果を使う。アクセス不能や対局の識別子不一致を拒否し、所有するスナップショットを復元する。 未検証範囲。公開範囲と履歴を同時に変更する専用テストはなく、閲覧者読み取り全体の整合性の契約を独立には確認していない。",
        "matchmaking-pairing/selection。選択処理は、期限切れの待機、同じ所有者、別プールを除外する。基準側をキュー追加時刻・UUID・所有者の順、相手側を差・時刻・UUID・所有者の順に並べる。",
        "matchmaking-pairing/windows。未評価のレーティングは実効値 1500 とする。許容差は 100 から始まり、30 秒が完全に経過するごとに 50 増え、上限は 400 となる。広い整数型で計算する。",
        "rating-application/worker-reconciliation。RatingWorker は不正入力と適用時の破損を隔離する。利用不能となった適用を一度再試行して不確かなコミットを照合し、リポジトリの結果を報告する。 未検証範囲。選択したエラー経路は検証しているが、全分岐を独立に網羅する検証や、WorkerId・WorkerPolicy・ClaimedRatingJob の専用コンストラクターテストはない。",
        "durable-event-dispatch/atomic-facts。受理記録と、固定された正規の Outbox イベントを一緒にコミットする。拒否と同一リクエストの再試行ではイベントを追加しない。",
        "durable-event-dispatch/ordered-identity。配信はキューの順序と安定した識別子 (game_id, seq, kind) を保持する。送信または処理済み更新の成否不明時には、未処理の仕事を飛ばさず、同じ識別子を重複配信する場合がある。",
        "durable-event-dispatch/confirmed-acceptance。購読者数 0 を含む非負整数の PUBLISH 応答を受け取った後にだけ、対象の一行を処理済みにする。これは通信先による受け入れの確認であり、購読者への到達やブローカーの永続保存の確認ではない。",
        "durable-event-dispatch/exclusive-fail-closed。一つの PostgreSQL セッションが配信ロックを所有する。そのセッションを失ったら処理を止め、配信を呼ぶ境界ではデータベースのトランザクションを開いたままにしない。",
        "durable-event-dispatch/process-dispatch。配信プロセスはキューと通信先を所有し、未処理イベントを定期的に確認する。配信の失敗または Ctrl-C で停止する。",
        "game-command-recovery/exact-retry。対局ごとのリクエスト識別子は、受理時にだけ確保する。参加者確認後、型付き操作・ユーザー・元の予想シーケンスが一致する再試行は、再起動後も古いシーケンスや終局の確認より先に元の受理結果を返す。受理済みの内容を変更すると競合する。",
        "game-command-recovery/sequence。新たに受理した操作ごとに、シーケンスを i64::MAX まで一度ずつ進める。対局ごとのアクター処理とリポジトリの CAS が、競合するサービスインスタンスを直列化し、競合に負けた側の状態を更新する。",
        "game-command-recovery/atomic-events。候補局面はコミット確認後にだけ公開する。メタデータ・受理記録・渡された正規イベントを一緒にコミットする。着手は MoveCommitted を生成し、詰みでは同一シーケンスに GameFinished を追加し、投了は GameFinished を生成する。再試行・拒否・復元では新しいイベントを生成しない。",
        "game-command-recovery/uncertain-recovery。永続コミットの後にエラーが返る場合がある。アクターは一致する保存済み受理記録と照合するか、後の永続状態の再読み込みが成功するまで自身の状態を無効化する。読み込み失敗時にキャッシュを使って変更を続けることはない。",
        "game-command-recovery/cancellation。対局ごとの 64 件のメールボックスが、キューに入る操作数を制限する。追加後の取消は実行を取り消さず、追加前の取消では実行しない。送信元が閉じた後も、キュー内の操作を処理して終了する。",
        "game-snapshots-access/snapshot-shape。信頼済み呼び出し元へのスナップショット取得が成功すると、永続シーケンス、固定された参加者、所有する盤面と 14 個の持ち駒カウンター、手番・王手・結果・合法手を返す。終局後も局面を保持し、合法手は空となる。",
        "game-snapshots-access/viewer-policy。公開対局はゲートウェイ経由の認証済み観戦者を許可する。非公開対局は参加者を許可する。閲覧者向けスナップショット・アクセス RPC は、アクセス不能な対局と存在しない対局を同じ NotFound として扱う。",
        "game-snapshots-access/recovery。スナップショットは永続バージョンを更新し、読み取り失敗時には状態を返さず、破損した履歴やメタデータを拒否する。再起動時には進行中と終局後のスナップショットを復元する。",
        "matchmaking-pairing/policy。組合せ処理は不適合な先頭を読み飛ばし、最も古い適合する基準側と最もレーティング差の小さい相手を選ぶ。同値の場合は時刻・UUID・所有者で順序を決め、別々のユーザーと同一プールを必須とし、双方の拡大した許容幅を適用する。",
        "matchmaking-pairing/server-ownership。サーバーが対局と固定された先手・後手の割り当てを決める。組合せの成立と二枚のチケットの遷移は一緒にコミットされ、同時に動く組合せ処理では一つだけが永続的な結果を確定する。",
        "matchmaking-pairing/expiry。レーティングのロック待ち中に失効した場合も含め、期限切れチケットは組み合わせられない。",
        "matchmaking-pairing/ready-before-play。組合せから 15 秒の共通期限内に双方が準備完了を送った場合だけ対局を作成する。期限ちょうどを含む失効は両者を Voided にし、勝敗・レーティングを作らず再参加を許す。準備確認の再送は期限を延長せず、新しい参加に影響しない。",
        "matchmaking-pairing/periodic-progress。実際の game-server は定期的に組合せ処理を行うため、追加の参加がなくても待機中の二人を組み合わせ、期限切れの準備待ちを無効にする。各バッチは開始プールを交互に替え、試行を 32 回に制限し、二つのプールがともに空か失敗した時点で止まる。",
        "matchmaking-tickets/owner-admission。所有者は対応するどちらのレーティングプールにも参加を要求できるが、プール全体で待機中・準備待ち・対局中の参加は一件に限られる。サーバーは参加時のレーティングを固定し、UUID による再試行の識別を保持する。",
        "matchmaking-tickets/durable-history。現在参加の取得と、ハートビートや元の操作の再試行による保存済み結果の取得は所有者に限定する。同一の再試行は保存済み結果を保持し、プールの変更は競合する。参加拒否と参加前の取消は永続化される。",
        "matchmaking-tickets/waiting-lifecycle。待機は、参加受け入れ時またはシーケンスが厳密に大きい最新の正のハートビートから 30 秒続く。同じか古いハートビートでは延長も復活もしない。取消は待機を終了させるが、組合せ済みの対局は取り消せない。",
        "matchmaking-tickets/browser-contract。ブラウザーの五つの組合せ操作は有効なセッションを必須とし、Host と変更時の Origin・same-site を確認する。呼び出し元の user_id を拒否し、安定したチケット・エラー JSON を返して no-store を適用する。OpenAPI は五つすべての操作を記述する。",
        "rating-application/terminal-durable-input。永続化されたレーティング対象対局の終局だけが、受理記録と Outbox と原子的にレーティングジョブを生成する。計算にはプールごとに固定された開始状態と確定した勝者を使う。",
        "rating-application/zero-sum-checked-elo。K32 Elo は未評価の両者を 1500 と扱い、先手の差分を一度だけ丸め、後手にはその符号を反転した差分を適用する。符号付きレーティングと完了対局数の算術を検査する。",
        "rating-application/atomic-once-ledger。有効な現在の取得権に基づく適用は、両者のレーティング更新、台帳一件の挿入、ジョブ完了、その対局自身の予約枠だけの解放を原子的に行う。一致する完了済み処理の再試行は、読み取りだけで結果を照合する。",
        "rating-application/fenced-retry。失効または更新済みの取得権では結果を変更できない。試行回数・リース期限・一時的失敗の待機時間を永続化し、破損や試行枠の使い切りでは予約枠を解放せず隔離する。",
        "rating-application/explicit-recovery。明示的な復旧は、入力の修復や固定された開始状態・レーティング・予約枠の変更をせずに、隔離ジョブの取得回数の枠を戻す。ワーカー実行ファイルは、定期実行と retry-quarantined の両モードを提供する。"
    ],
    external: &[],
    operations: &[],
    entrypoints: &[
        trace::EntryPoint {
            name: "永続 Outbox の一件配信",
            source: trace::Source {
                path: "crates/game-application/src/events.rs",
                needle: "pub async fn publish_next(&mut self) -> Result<bool, DispatchError> {"
            }
        },
        trace::EntryPoint {
            name: "対局アクターと操作の復旧",
            source: trace::Source {
                path: "crates/game-application/src/actor.rs",
                needle: "struct Actor {"
            }
        },
        trace::EntryPoint {
            name: "レートジョブの処理",
            source: trace::Source {
                path: "crates/game-application/src/rating.rs",
                needle: "pub async fn tick(&self, owner: &WorkerId) -> Result<TickOutcome, RatingWorkerError> {"
            }
        }
    ]
});
