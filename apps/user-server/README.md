# user-server の起動手順

Google ログインを使う HTTP アカウントサービスを起動する。

API の仕様を出力するには、[API ドキュメントの生成手順](../../README.md#api-ドキュメントを生成する)に従う。
仕様、実装、テストの対応は[HTML viewer の手順](../../docs/how-to/check-traceability.md#html-viewer-を開く)に従って確認する。

## Google ログインを設定する

サービスを起動する前に Google クライアントを設定する。

1. 種類が **Web application** の Google OAuth クライアントを作成する。
2. `USER_PUBLIC_ORIGIN` に `/auth/google/callback` を付けたリダイレクト URI を登録する。
   ローカル開発では `http://localhost:8080/auth/google/callback` を使う。
3. Google アプリが **Testing** の場合は、開発用アカウントをテストユーザーに追加する。

プロバイダー側の設定は [Google OpenID Connect のドキュメント](https://developers.google.com/identity/openid-connect/openid-connect)に記述されている。

環境変数は [.env.example](../../.env.example) を元に設定する。
追加の設定と制約は [ServerConfig](../../crates/user-http/src/config.rs) で確認する。

## ローカルで起動する

コマンドはリポジトリのルートで実行する。
PostgreSQL を起動し、`.env` がまだなければ設定テンプレートをコピーする。

```sh
docker compose up -d --wait postgres
if [ ! -e .env ]; then cp .env.example .env; fi
```

`.env` の `GOOGLE_CLIENT_ID` と `GOOGLE_CLIENT_SECRET` を、作成したクライアントの認証情報に置き換える。
別の PostgreSQL を使う場合は、専用データベースとロールを事前に作成し、`USER_DATABASE_URL` を変更する。
ロールにはデータベースの `CREATE` 権限と、`public` 内にオブジェクトを作成する権限を付与する。
既存スキーマを検証するため、これらの権限は毎回の起動時にも必要になる。
スーパーユーザー権限や `CREATEROLE` 権限の付与は不要となる。
ローカル環境の作成手順は[初期設定 SQL](../../scripts/init-postgres.sql)を参照する。

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
ログイン後に `/me` へ移動し、プロフィールの JSON が表示されることを確認する。
リダイレクト URI とブラウザーで開く URL のホストを揃える。
`localhost` と `127.0.0.1` を混在させると、ログイン用 Cookie を共有できない。

## Docker Compose で起動する

Compose は、リポジトリのルートにある `.env` から Google の認証情報と公開オリジンを読み込む。
次のコマンドで起動する。

```sh
docker compose --profile auth up --build -d user-server
```

コンテナーの待ち受けアドレスと接続先データベースには、`.env` の値ではなく [compose.yaml](../../compose.yaml) の設定を使う。
ゲートウェイと同時に使う場合は、[リアルタイムゲートウェイの起動手順](../realtime-gateway/README.md)に従う。

## 本番環境に配置する

公開 HTTPS オリジンを設定し、それに対応するコールバック URL を Google に登録する。
ループバック HTTP 開発モードを無効にする。
リバースプロキシで TLS を終端し、内部 HTTP 待ち受けへのアクセスを制限する。
ブラウザーの Origin ヘッダーを保持すること。サービスは認証判断に `X-Forwarded-*` ヘッダーを使わない。

ユーザーデータベースを永続ストレージに置く。
認証データを含むデータベースとバックアップへのアクセスを運用者に制限する。
`.env` と Google の認証情報をバージョン管理に含めず、ファイルへのアクセスを運用者に制限する。
コールバックのクエリ文字列、Cookie、OAuth レスポンスをプロキシのアクセスログに記録しないこと。
開発用 HTTP 待ち受けを信頼できないネットワークに公開しないこと。

Google のディスカバリー、署名鍵、トークン取得先への送信通信を許可する。

## 動作を検証する

リポジトリのルートでアカウントサービスのテストを実行する。

```sh
./scripts/test-postgres.sh cargo test --locked -p user-id -p user-application -p user-persistence -p user-http -p user-server
```

実際の Google ログインは、上記のローカル起動手順に従ってブラウザーで確認する。
