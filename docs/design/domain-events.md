# Domain EventとTransactional Outboxの設計

本書は、確定した対局操作をPostgreSQLへ保存し、独立したPublisherからCore NATSへ送る現行設計を説明する。
全体構成と将来の分散設計は[アーキテクチャ設計書](architecture.md)、実行手順は[README](../../README.md)を参照する。
時計、時間切れ、Shard、Lease、Game Router、Gatewayは未実装である。

## 保存前にイベントを固定する

`game-application`がDomain Eventの生成と配信ユースケースを所有する。
Actorは局面のコピー上で合法性と参加者を検証し、受理記録`Receipt`を作る。
続いて`StagedGameChange::new(game_id, receipt)`でイベントを生成し、`GameRepository::commit_change(&change)`へ渡す。

`StagedGameChange`は対局ID、受理記録、正規のイベント列を非公開フィールドで一体として保持する。
公開アクセサーは共有参照だけを返すため、保存アダプターは受理記録と異なるイベント列へ差し替えられない。
コンストラクターは連番の範囲と連続性、操作と終局理由の整合性も検証する。
合法手の検証はActor、保存時の参加者・version・一意制約の検証はRepositoryが担う。

受理した操作ごとのイベントは次のとおりである。

| 操作 | イベントの順序 |
|---|---|
| 通常の着手 | `MoveCommitted` |
| 詰みの着手 | `MoveCommitted`、`GameFinished` |
| 投了 | `GameFinished` |

イベントの識別子は`(game_id, seq, event_kind)`である。
詰みの2イベントは同じ`seq`を持ち、種別で区別する。元の`request_id`と`user_id`も保持する。
対局作成、拒否、受理済み要求の完全一致での再送、履歴復元は新しいイベントを作らない。
型と生成規則は[events.rs](../../crates/game-application/src/events.rs)、呼出箇所は[actor.rs](../../crates/game-application/src/actor.rs)にある。

## 受理記録とイベントを同時に確定する

`game-persistence`は対局メタデータのversion条件付き更新、受理記録、渡されたイベント全件を同じPostgreSQLトランザクションで保存する。
イベントを生成・再計算せず、受け取った順序と内容を保存する。
これにより、対局操作だけが確定して通知対象が欠落する状態を防ぐ。
Actorは保存成功を確認してからコピーした局面をキャッシュへ反映し、応答する。NATSへ直接送信しない。

保存エラーはロールバックを意味しない。
Actorは履歴を再読込し、同じ要求の受理記録があれば元の応答を返す。
結果が不明なままならキャッシュを無効化し、`Unavailable`を返す。
呼出元は元の`expected_seq`、同じ内容、同じ`request_id`で再送する。
この照合により、確定後の応答喪失でも受理記録とOutboxを重複追加しない。
契約は[GameRepository](../../crates/game-application/src/repository.rs)にある。

## 保存形式と通信形式を分ける

Outbox行は`encoding_version=1`の型付きフィールドを保持する。JSONバイト列は保存しない。
未配信イベントを可変の対局履歴から再生成せず、固定した行から復元する。
未知の保存形式や不正なフィールドはエラーにし、先頭行を飛ばさない。

`game-messaging`がSubjectとJSONを所有する。
Subjectは`shogi.games.<hex-game-id>.events`で、対局IDのUTF-8バイト列を小文字hexへ変換する。
これにより、ドット、NATSのワイルドカード、Unicode、NULを含むIDもSubjectの構文を壊さない。
JSONの`game_id`は元の文字列を保持する。

通信形式は`schema_version=1`で、`event_id`は`<hex-game-id>:<seq>:<type>`である。
`type`は`move_committed`または`game_finished`となる。
共通項目は`game_id`、`seq`、`request_id`、`user_id`で、着手にはUSIの`movement`と`in_check`を加える。
`result`は継続中ならnull、終局なら`winner`と`reason`を持つ。
勝者は`black`または`white`、理由は`checkmate`または`resignation`である。
将来のエンコーダー変更でも、保存形式1から通信形式1への対応を維持する。
定義と通信形式のテストは[game-messaging](../../crates/game-messaging/src/lib.rs)にある。

新規DBはスキーマversion 5を原子的に作成する。
空の対局DBは現行スキーマで初期化し、受理した操作からOutboxイベントを保存する。既存DBは現行形式を検証し、非対応の形式は変更せず起動を拒否する。
必須テーブルの欠落、不正なスキーマ台帳、孤立したOutbox、未知のversionは起動時に拒否する。

## 配信確認と重複の限界

`OutboxPublisher`は最古の未配信行を1件取得し、送信確認後にその行だけを配信済みとする。
読取・送信・記録の失敗では未配信の先頭を飛ばさない。配信済み行は保持する。
ネットワーク待機中にDBトランザクションを保持しないため、NATS停止中も対局操作を保存できる。

NATSアダプターは実際のイベントSubjectを購読してから送信し、同じ接続でSubjectとペイロードが一致するechoを待つ。
イベントSubjectに対するPublish権限とSubscribe権限が必要である。
ソケットの`flush`だけでは、サーバーがイベントを処理したと確認できない。
別のprivate inboxのechoも、イベントSubjectに対する権限拒否を検出できない。
`server_barrier`は検証クライアントの購読準備に使うが、イベントの受入確認には使わない。

初回接続後は再接続先の選択を待機させ、置換後の接続のechoで古い送信を確認しない。
配信失敗時はWorkerを終了し、Composeの再起動で未配信行から再開する。
[プロセスのmain](../../apps/outbox-publisher/src/main.rs)はTokio runtimeを破棄してからキューを解放する。
これにより、バックグラウンド通信が停止するまでPublisherの排他を維持する。

単一Publisherでもexactly-once配信にはならない。
送信成功後の配信済み記録の失敗や、その間のクラッシュでは同じ識別子を再送し得る。
echoは一致するイベントの確認であり、特定の送信操作や他の購読者の受信・永続化を保証しない。
Core NATSは切断中の購読者向けにイベントを保持しない。
購読者は完全なイベント識別子で重複を除き、欠落時はSnapshotで再同期する必要がある。
通信障害をまたぐ遅延重複や到着順の逆転も許容する。購読者側の再同期は未実装である。

## PostgreSQLで単一Publisherを排他する

`PostgresOutbox`は専用接続でセッションadvisory lockを保持する。
同じ対局DBに対するロック取得に失敗したプロセスは起動を終了する。
全サービスは同じ対局DBへ接続するが、同一ホストへの配置や共有ファイルは不要である。
待機系への自動昇格や並列Workerは実装していない。

キューの読取と配信済み記録には、ロックを保持する専用接続を使う。
この接続を失ったPublisherは処理を継続しない。
NATS通信待ちにDBトランザクションを保持せず、対局の書込みは継続できる。
終了時は通信処理を停止してから接続を閉じ、所有権を解放する。

advisory lockは単一Publisherの排他であり、exactly-once配信の保証ではない。
並列配信には取得・担当管理・順序保証を別途設計する必要がある。
保持行の掃除、Gateway配信、Publisherの高可用性は将来の対象である。

## 契約を確認する場所

検証は固定の成功件数ではなく、現行のテストと再実行できる手順で確認する。

- [イベントと配信ユースケースのテスト](../../crates/game-application/src/events.rs)が生成順、識別子、失敗時の再送を確認する。
- [Actorのテスト](../../crates/game-application/src/actor/tests.rs)が保存前の生成、再送・拒否時の呼出抑止を確認する。
- [Outboxの永続化テスト](../../crates/game-persistence/tests/outbox.rs)が原子性、固定内容、再起動後の保持、不確定commit、プロセス排他を確認する。
- [NATSアダプターのテスト](../../crates/game-messaging/src/lib.rs)が通信形式、実イベントのecho、権限拒否、再接続の抑止を確認する。
- [Compose検証スクリプト](../../scripts/prove-outbox.sh)が実際の配信、Publisher停止中の蓄積、サーバー再作成、NATS停止後の復旧を確認する。前提条件と実行方法は[README](../../README.md#publish-committed-game-events)にある。
