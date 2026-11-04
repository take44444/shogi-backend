# Domain EventとTransactional Outboxの設計

本書は、確定した対局操作をPostgreSQLへ保存し、独立したPublisherからValkey Pub/Subへ一時通知を送る現行設計を説明する。
更新日は2026-11-05である。全体構成と将来の分散設計は[アーキテクチャ設計書](architecture.md)、実行手順は[README](../../README.md)を参照する。
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
Actorは保存成功を確認してからコピーした局面をキャッシュへ反映し、応答する。Valkeyへ直接送信しない。

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

`game-messaging`がChannelとJSONを所有する。
Channelは`shogi.games.<hex-game-id>.events`で、対局IDのUTF-8バイト列を小文字hexへ変換する。
これにより、ドット、ワイルドカード、Unicode、NULを含むIDも一意なChannelへ変換できる。
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

## 一時通知の受入確認と重複の限界

`OutboxPublisher`は最古の未配信行を1件取得し、`NotificationPublisher`の成功後にその行だけを配信済みとする。
成功はブローカーによる一時通知の受入を意味する。購読者の受信、永続化、処理完了は保証しない。
読取・送信・記録の失敗では未配信の先頭を飛ばさない。配信済み行は保持する。
ネットワーク待機中にDBトランザクションを保持しないため、Valkey停止中も対局操作を保存できる。

`ValkeyEventPublisher`は`redis` crateのRedis互換接続で、イベントのChannelへ`PUBLISH`を送る。
サーバーの非負整数応答を受け取ると成功とし、購読者数が0でも成功する。
権限拒否等のサーバーエラー、通信失敗、タイムアウトでは未配信のままWorkerを終了する。
失敗した接続は破棄し、自動再接続は行わない。Composeの再起動で未配信行から再開する。
設定は`VALKEY_URL`で、既定値は`redis://127.0.0.1:6379`である。

[プロセスのmain](../../apps/outbox-publisher/src/main.rs)はTokio runtimeを破棄してからキューを解放する。
これにより、ローカルの通信処理を停止してからPublisherの所有権を解放する。
ただし、送信済みの`PUBLISH`がValkey上で後から実行されることまでは防げない。
PostgreSQLのセッション喪失後に送信中の通知が届き、未配信行を再取得した別Publisherの通知と重複・順序逆転する可能性がある。

単一Publisherでもexactly-once配信にはならない。
送信成功後の配信済み記録の失敗や、その間のクラッシュでは同じ識別子を再送し得る。
Valkey Pub/Subは購読者不在や切断中の通知を保持しない。
購読者は完全なイベント識別子で重複を除き、欠落時はSnapshotで再同期する必要がある。
最後の通知の欠落は後続の連番で検知できないため、将来のGatewayでは定期照合も行う。
通信障害をまたぐ遅延重複や到着順の逆転も許容する。購読者側の再同期は未実装である。

## 一時通知と永続ジョブの状態を分ける

現行の`outbox_events.published`は一時通知の受入記録だけである。
このフラグをレーティング更新、AI解析、その他のConsumerの完了判定に使わない。
Gateway、Rating Worker、AI Worker、永続ジョブキューは未実装である。

将来の永続処理には、Consumer別のPostgreSQLジョブと完了状態を設ける。
ジョブは一意キー、所有者・世代を持つ期限付きclaim、再試行時刻・回数、隔離状態を持つ。
Workerは短いトランザクションでclaimし、長い処理の間はトランザクションを保持しない。
完了は処理結果のDB Commitを根拠とする。
同じDB境界の業務更新と完了記録は、有効なclaimを検証した同じトランザクションに含める。
別DB境界の完了記録は業務Commit後に行う。
再試行は業務側の一意制約・適用台帳でも冪等にする。

Game側の確定データとジョブを同じトランザクションで保存できる場合は直接保存する。
別DB境界では、送信元の専用Outboxから受信側の永続Inboxへ同じIDで渡す。
受信側はInboxとジョブを原子的に保存し、その耐久的な受入後に確認を返す。
送信側は確認後にだけ、その宛先への受入済み状態を記録する。
この状態も業務処理の完了とは区別し、一時通知用の`published`とは共有しない。
Pub/Subの購読を永続ジョブへの中継元にしない。Ratingの順序・照合とAIの処理境界は[アーキテクチャ設計書](architecture.md#9-postgresqlとvalkeyの使い分け)を参照する。

## PostgreSQLで単一Publisherを排他する

`PostgresOutbox`は専用接続でセッションadvisory lockを保持する。
同じ対局DBに対するロック取得に失敗したプロセスは起動を終了する。
Game ServerとPublisherは同じ対局DBへ接続するが、同一ホストへの配置や共有ファイルは不要である。
待機系への自動昇格や並列Workerは実装していない。

キューの読取と配信済み記録には、ロックを保持する専用接続を使う。
この接続を失うと以後のキュー操作が失敗し、Publisherはその失敗を検知して終了する。
Valkey通信待ちにDBトランザクションを保持せず、対局の書込みは継続できる。
終了時は通信処理を停止してから接続を閉じ、所有権を解放する。

advisory lockはキュー操作の単一Publisher排他であり、送信済み通知の遠隔実行を止めるFencingやexactly-once配信の保証ではない。
並列配信には取得・担当管理・順序保証を別途設計する必要がある。
保持行の掃除、Gateway配信、Publisherの高可用性は将来の対象である。

## 契約を確認する場所

検証は固定の成功件数ではなく、現行のテストと再実行できる手順で確認する。

- [イベントと配信ユースケースのテスト](../../crates/game-application/src/events.rs)が生成順、識別子、失敗時の再送を確認する。
- [Actorのテスト](../../crates/game-application/src/actor/tests.rs)が保存前の生成、再送・拒否時の呼出抑止を確認する。
- [Outboxの永続化テスト](../../crates/game-persistence/tests/outbox.rs)が原子性、固定内容、再起動後の保持、不確定commit、プロセス排他を確認する。
- [Valkeyアダプターのテスト](../../crates/game-messaging/src/lib.rs)が通信形式、`PUBLISH`の整数応答、購読者不在、エラー時の未配信保持、再接続の抑止を確認する。
- [Compose検証スクリプト](../../scripts/prove-outbox.sh)が実際の配信、Publisher停止中の蓄積、サーバー再作成、Valkey停止後の復旧を確認する。前提条件と実行方法は[README](../../README.md#publish-committed-game-events)にある。
