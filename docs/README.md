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
- [テストを配置する](how-to/organize-tests.md)

## 設計

- [対局時計の設計理由と運用上の制約](design/game-clock.md)
- [Google による本人確認とアプリケーションのセッションを分ける](design/browser-authentication.md)
- [対局イベントの配信と永続処理](design/event-processing.md)
- [拡張時の設計と運用上の検証課題](design/unfinished-work.md)
- [マッチメイキングの拡張時に決めること](design/matchmaking.md)
