# ドキュメント

採用済みの重要な判断と理由はADRに記録する。未完成の機能と将来の設計案は、このディレクトリで管理する。
現在の契約は型・公開API・DB制約・権限設定と、それらを検証するテスト・CIで管理する。

## 採用済みの判断

- [ユーザーDBを対局DBから分離する](adr/0001-separate-user-database.md)
- [Googleによる本人確認とアプリケーションのセッションを分ける](adr/0002-browser-authentication.md)
- [対局イベントを受理時に確定し、一時通知と永続処理を分ける](adr/0003-game-event-delivery.md)

## 未完成の機能と将来の設計案

[未完成の機能と未検証の範囲](design/unfinished-work.md)に、実装と検証が残る項目をまとめる。
[イベント配信と永続ジョブの将来設計](design/event-processing-plan.md)は、導入時の方針を記述する。現行実装の保証ではない。

## 操作仕様と運用手順

設定・起動・検証手順は[リポジトリのREADME](../README.md)、[user-serverのREADME](../apps/user-server/README.md)、[player-serverのREADME](../apps/player-server/README.md)に置く。
操作仕様は[公開棋譜HTTP API](../crates/player-http/README.md)、[ユーザーHTTP API](../crates/user-http/README.md)、[対局gRPC API](../crates/game-grpc/README.md)を参照する。
