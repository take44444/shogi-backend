# ドキュメント

このディレクトリには、採用済みの判断、未完成の機能、将来の設計案を置く。
現行の仕様は、型、公開 API、DB 制約、権限設定と、それらを検証するテストや CI に対応する。

## 採用済みの判断

重要な判断と理由は、次の ADR に記録する。

- [ユーザー DB を対局 DB から分離する](adr/0001-separate-user-database.md)
- [Google による本人確認とアプリケーションのセッションを分ける](adr/0002-browser-authentication.md)
- [対局イベントを受理時に確定し、一時通知と永続処理を分ける](adr/0003-game-event-delivery.md)

## 未完成の機能と将来の設計案

次の文書には、実装や検証が残る項目を記述する。将来の設計案は、現行実装の保証ではない。

- [未完成の機能と未検証の範囲](design/unfinished-work.md)
- [イベント配信と永続ジョブの将来設計](design/event-processing-plan.md)
- [マッチメイキングの未検証事項と拡張時の制約](design/matchmaking.md)

## 起動と運用の手順

起動、設定、検証、復旧の手順は、次の文書にまとめる。

- [リポジトリ全体の起動と検証](../README.md)
- [ユーザーサービスの起動](../apps/user-server/README.md)
- [公開棋譜サービスの起動](../apps/player-server/README.md)
- [リアルタイムゲートウェイ の起動とブラウザーの接続設定](how-to/run-realtime-gateway.md)
- [レートワーカーの起動と隔離ジョブの再試行](../apps/rating-worker-server/README.md)

## 設定と API の参照仕様

環境変数、リクエスト、レスポンス、処理上の制限は、次の文書に記述する。

- [サービスの環境変数と必要な権限](reference/service-configuration.md)
- [公開棋譜とレートの HTTP API](../crates/player-http/README.md)
- [ユーザー HTTP API](../crates/user-http/README.md)
- [対局 gRPC API](../crates/game-grpc/README.md)
- [マッチメイキング HTTP API と対局 WebSocket](../apps/realtime-gateway/README.md)
- [Valkey の対局通知](reference/game-notifications.md)
