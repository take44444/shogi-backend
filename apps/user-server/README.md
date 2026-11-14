# user-server の起動手順

Google ログイン、プロフィール、ブラウザーセッションを提供する HTTP アカウントサービスを起動する。
専用の PostgreSQL データベースを使い、`game-server` とは独立して動作する。

リクエスト、Cookie、エラーは [HTTP API の参照仕様](../../crates/user-http/README.md)に記述する。
選択した方式の理由は[認証方式の ADR](../../docs/adr/0002-browser-authentication.md)に記録する。
残りの機能と検証範囲は[未完成の機能](../../docs/design/unfinished-work.md)にまとめる。

## Google ログインを設定する

サービスを起動する前に Google クライアントを設定する。

1. 種類が **Web application** の Google OAuth クライアントを作成する。
2. `USER_PUBLIC_ORIGIN` に `/auth/google/callback` を付けたリダイレクト URI を登録する。
   ローカル開発では `http://localhost:8080/auth/google/callback` を使う。
3. Google アプリが **Testing** の場合は、開発用アカウントをテストユーザーに追加する。

サービスは `openid profile` スコープを要求する。
プロバイダー側の設定は [Google OpenID Connect のドキュメント](https://developers.google.com/identity/openid-connect/openid-connect)に記述されている。

[ユーザーサービスの設定](../../docs/reference/service-configuration.md#ユーザーサービス)に従って環境変数を指定する。
下記のローカル設定例には必須の値を含めている。

## ローカルで起動する

コマンドはリポジトリのルートで実行する。
PostgreSQL を起動し、`.env` がまだなければ設定テンプレートをコピーする。

```sh
docker compose up -d --wait postgres
if [ ! -e .env ]; then cp .env.example .env; fi
```

`.env` に Google の認証情報を設定する。ローカル設定の例を示す。

```dotenv
GOOGLE_CLIENT_ID='your-client-id.apps.googleusercontent.com'
GOOGLE_CLIENT_SECRET='your-client-secret'
USER_PUBLIC_ORIGIN='http://localhost:8080'
USER_ALLOW_LOOPBACK_HTTP='true'
USER_BIND_ADDR='127.0.0.1:8080'
USER_DATABASE_URL='postgres://shogi:shogi@localhost:5432/users'
```

アプリケーションと Cargo は `.env` を自動で読み込まない。
サーバーの起動前に、現在のシェルで設定値をエクスポートする。
設定を変更した場合は、ファイルを読み直してプロセスを再起動すること。

```sh
set -a
. ./.env
set +a
cargo run --locked -p user-server
```

起動時に `user HTTP server listening on 127.0.0.1:8080` と出力する。
`http://localhost:8080/auth/google/start` を開いてログインする。
コールバックが成功すると `/me` に移動し、認証済みプロフィールを JSON で返す。
初回ログインでアカウントを作成し、以後のログインでは同じ内部ユーザー ID を使う。

## Docker Compose で起動する

Compose は、リポジトリのルートにある `.env` から Google の認証情報と公開オリジンを読み込む。
次のコマンドで起動する。

```sh
docker compose --profile auth up --build -d user-server
```

`auth` プロファイルは任意で追加する。既定の Compose 構成は対局関連のサービスを起動する。
ユーザーサービスのコンテナー内待ち受けアドレスは `0.0.0.0:8080` となり、ホストのループバックアドレスでポートを公開する。
PostgreSQL 内の専用データベース `users` に接続し、データは `pg-data` に保持する。
コンテナーの待ち受けアドレスと接続先データベースには、`.env` の値ではなく [compose.yaml](../../compose.yaml) の設定を使う。

## 本番環境に配置する

公開 HTTPS オリジンを設定し、それに対応するコールバック URL を Google に登録する。
ループバック HTTP 開発モードを無効にする。
リバースプロキシで TLS を終端し、内部 HTTP 待ち受けへのアクセスを制限する。
ブラウザーの Origin ヘッダーを保持すること。サービスは認証判断に `X-Forwarded-*` ヘッダーを使わない。

ユーザーデータベースを永続ストレージに置く。
`.env` と Google の認証情報をバージョン管理に含めず、ファイルへのアクセスを運用者に制限する。
コールバックのクエリ文字列、Cookie、OAuth レスポンスをプロキシのアクセスログに記録しないこと。
開発用 HTTP 待ち受けを信頼できないネットワークに公開しないこと。

[アカウント保存層の参照仕様](../../crates/user-persistence/README.md#起動時のスキーマ処理)に従ってデータベースロールに権限を付与する。
Google のディスカバリー、署名鍵、トークン取得先への送信通信を許可する。
起動時の動作とタイムアウトは[ユーザーサービスの設定](../../docs/reference/service-configuration.md#ユーザーサービス)に記述する。

## 動作を検証する

リポジトリのルートでアカウントサービスのテストを実行する。

```sh
./scripts/test-postgres.sh cargo test --locked -p user-id -p user-application -p user-persistence -p user-http -p user-server
```

テストは、署名付き ID トークンの検証、PKCE コード交換、ブラウザーに結び付いたログイン試行、
同時登録、プロフィール更新、CSRF 検査、セッションの期限切れ、再起動、ログアウトを対象とする。
ローカル HTTP テストは模擬プロバイダーを使う。
実際の Google ログインには、設定済み OAuth クライアントとブラウザー操作が必要になる。

`GET /openapi.json` は OpenAPI 3.1 の仕様を返す。
サーバーを起動せずに出力する場合は、[API ドキュメントの生成手順](../../README.md#api-ドキュメントを生成する)に従うこと。
