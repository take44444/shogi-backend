# user-http の参照仕様

`router` は Google ログインとアカウント操作のルートを提供する。
`GoogleProvider` は `openidconnect` 4.1 を通じて固定の Google 発行者を使う。
コード交換ごとにディスカバリー情報と署名鍵を取得し直す。
各送信リクエストにはタイムアウトがあり、リダイレクトを追跡しない。

## ルート

| メソッド | パス | 結果 |
| --- | --- | --- |
| GET | `/auth/google/start` | ブラウザーに結び付いた有効期限 10 分のログイン試行を作成し、Google に移動する。 |
| GET | `/auth/google/callback` | 試行を消費し、Google の本人確認情報を検証してセッションを作成し、`/me` に移動する。 |
| GET | `/me` | 認証済みプロフィールを返す。 |
| PATCH | `/me` | 認証済みユーザーの表示名を変更する。一致する `Origin` が必要。 |
| POST | `/auth/logout` | 現在のセッションを失効させ、Cookie を削除する。一致する `Origin` が必要。 |
| GET | `/openapi.json` | OpenAPI 仕様を返す。 |

## 本人確認とブラウザーセッション

公開 API は呼び出し元が選んだ内部ユーザー ID を受け付けず、メールアドレスで外部 ID を結び付けない。
`GoogleProvider` はトークン検証後に Google のアクセストークンとリフレッシュトークンを破棄する。
セッションには無作為に生成した不透明なブラウザー Cookie を使い、PostgreSQL には SHA-256 ハッシュを保存する。
サーバー側の有効期限は固定となる。
ログインが成功すると、現在のブラウザーセッションを新しいセッションに置き換える。
すべてのレスポンスに `Cache-Control: no-store`、`Referrer-Policy: no-referrer`、`X-Content-Type-Options: nosniff` を付ける。

本番用 Cookie はホスト専用の `__Host-user_session` と `__Host-user_login` となる。
属性は `Secure`、`HttpOnly`、`SameSite=Lax`、`Path=/` となる。
明示的に有効化したループバック HTTP 開発モードでは、`Secure` のない `user_session` と `user_login` を使う。

## 入力検証とエラー

`PATCH /me` とログアウトは、`Sec-Fetch-Site` ヘッダーがある場合に `Sec-Fetch-Site: same-origin` を要求する。
ログアウトはセッション Cookie の欠落や不正を許容する。
コールバックのクエリ解析と JSON 抽出の失敗は、Axum のプレーンテキスト応答となる。
表示名変更は、不正な JSON に 400、本文の容量超過に 413、JSON の Content-Type の欠落や非対応の値に 415、
構造の不正に 422 を返す。
アプリケーションエラーは `error` コードを含む JSON となり、表示名の不正は 400 を返す。
`DisplayName::new` は前後の空白を取り除いた後、空でないこと、80 Unicode 文字以内であること、制御文字を含まないことを検証する。

Google クライアント設定、環境変数、配置、実プロバイダーでのログイン確認は
[ユーザーサービスの手順](../../apps/user-server/README.md)に記述する。
予定しているアカウント機能と対局認証の統合は[未完成の機能](../../docs/design/unfinished-work.md)にまとめる。

## OpenAPI

`GET /openapi.json` は OpenAPI 3.1 の仕様を返す。
公開関数 `user_http::openapi()` はデータベース、サービス設定、プロバイダーへの接続を必要としない。
ハンドラーの注釈、serde の DTO、単一の `OpenApiRouter` 登録処理が、実行時ルートと仕様の両方を定義する。
ハンドラーは `api_router()` 内の `routes!` と、各ハンドラーの隣に置く `#[utoipa::path]` 注釈を使う。
仕様用のパス一覧を別に管理することはない。

HTTP 仕様の出力は [API ドキュメントの生成手順](../../README.md#api-ドキュメントを生成する)に記述する。
生成した JSON はビルド成果物となる。

## テスト範囲

テストは模擬の本人確認プロバイダーを使い、認証、セッション、プロフィール変更、生成した OpenAPI の契約を検証する。
実行コマンドは[ワークスペースの検証手順](../../README.md#ワークスペースを検証する)に記述する。
