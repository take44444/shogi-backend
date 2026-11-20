# ドキュメント

コードから抽出したコンポーネント境界の呼出とテスト対象は[HTML viewer の手順](how-to/check-traceability.md)に従って確認する。
API のフィールドや型は OpenAPI、[対局プロトコル](../proto/game.proto)、rustdoc を参照する。
生成方法は[ワークスペースの手順](../README.md#api-ドキュメントを生成する)にまとめる。

## 開発と運用

- [起動とワークスペース全体の検証](../README.md)
- [ユーザーサービスと Google ログインの設定](../apps/user-server/README.md)
- [公開データサービスの起動と読み取りロールの準備](../apps/player-server/README.md)
- [ブラウザー向けゲートウェイの起動](../apps/realtime-gateway/README.md)
- [レートワーカーと隔離された対局の再試行](../apps/rating-worker-server/README.md)
- [コンポーネント境界とテスト対象の生成](how-to/check-traceability.md)

## 設計判断

- [ユーザー DB を対局 DB から分離する](adr/0001-separate-user-database.md)
- [Google による本人確認とアプリケーションのセッションを分ける](adr/0002-browser-authentication.md)
- [対局イベントを受理時に確定し、一時通知と永続処理を分ける](adr/0003-game-event-delivery.md)

## 将来の設計

- [未実装の機能と運用上の検証課題](design/unfinished-work.md)
- [イベント配信と永続ジョブの将来設計](design/event-processing-plan.md)
- [マッチメイキングの拡張時に決めること](design/matchmaking.md)
