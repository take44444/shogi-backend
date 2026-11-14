# 対局通知の参照仕様

Outbox 配信プロセスは Valkey Pub/Sub で一時通知を送る。
対局操作は、配信プロセスやブローカーを待たずにコミットする。

## チャネルとイベント識別子

各対局は `shogi.games.<hex-game-id>.events` チャネルを使う。
16 進数部分は、対局 ID の全 UTF-8 バイトを小文字で符号化する。
イベント ID は、16 進数の対局 ID、シーケンス番号、イベント種別をコロンで連結した値となる。

イベント種別は `move_committed` と `game_finished` となる。
詰みの着手は同じシーケンス番号で両方を生成し、着手イベントを先に送る。

## JSON の通知形式

[バージョン 1 のエンコーダー](../../crates/game-messaging/src/lib.rs)が次のフィールドを定義する。

| フィールド | 値 |
| --- | --- |
| `schema_version` | `1` |
| `event_id` | `<hex-game-id>:<seq>:<type>` 形式の安定した識別子 |
| `type` | `move_committed` または `game_finished` |
| `game_id` | 元の対局 ID |
| `seq` | JSON 数値で表す、受理した操作のシーケンス番号 |
| `request_id` | 受理したリクエスト ID |
| `user_id` | 操作を送ったユーザー |
| `movement` | 正規の USI 着手。`move_committed` だけに含む。 |
| `in_check` | 真偽値。`move_committed` だけに含む。 |
| `result` | 進行中は `null`。終局時は `winner` と `reason` を持つオブジェクト。 |

勝者は `black` と `white`、終局理由は `checkmate` と `resignation` となる。
バージョン 1 のエンコーダーを変更しても、この対応を維持する。
[ブラウザー WebSocket の契約](../../apps/realtime-gateway/README.md#websocket-の契約)は別のメッセージ形式を使い、
シーケンス値を 10 進数の文字列で表す。

## 配信保証

ブローカーによる受け入れは、購読者への到達や処理を保証しない。
切断中の購読者は通知を受信できない。
送信や配信済み更新の結果が不明になると、通知が重複することがある。
処理側は完全なイベント識別子で重複を除き、欠落した状態をスナップショットから回復する。

配信の失敗やタイムアウトが起きるとワーカーは停止し、次の配信試行で復旧する。
Compose はワーカーを再起動する。
配信済みの行は残り、処理側の完了ではなくブローカーの受け入れを記録する。

[アプリケーションの契約](../../crates/game-application/README.md#イベント配信)はイベント生成を定義する。
[Outbox の所有権](../../crates/game-persistence/README.md#outbox-の所有権)は順序と配信プロセスの障害を記述する。
選択の理由は[イベント配信の ADR](../adr/0003-game-event-delivery.md)に記録する。
