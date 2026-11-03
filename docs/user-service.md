# Google ログインを動かす

この手順で、Google ログイン、プロフィール取得、表示名変更、ログアウトを独立した HTTP サービスとして起動します。

## Google のクライアントを作成する

1. [Google Cloud Console](https://console.cloud.google.com/) でプロジェクトを選びます。
2. Google Auth Platform の Branding、Audience、Data Access を設定します。
3. アプリが Testing の場合は、ログインに使う Google アカウントをテストユーザーに追加します。
4. Clients で OAuth クライアントを作成します。アプリケーションの種類は **Web application** にします。
5. Authorized redirect URIs に `http://localhost:8080/auth/google/callback` を登録します。
6. クライアント ID とクライアントシークレットを、次の手順で `.env` に設定します。`.env` は Git の追跡対象外です。

リダイレクト URI は `USER_PUBLIC_ORIGIN` に `/auth/google/callback` を加えた値と完全に一致させます。本番では `https://users.example.com/auth/google/callback` のような HTTPS の URI を登録します。

参考資料は [Google の OpenID Connect](https://developers.google.com/identity/openid-connect/openid-connect) です。

## ローカルで起動する

リポジトリのルートで、設定例を `.env` にコピーします。すでに `.env` がある場合は、既存のファイルを編集します。

```sh
cp .env.example .env
```

`.env` に次の値を記載し、Google のクライアント ID とシークレットを実際の値に置き換えます。値はシングルクォートで囲みます。

```dotenv
USER_PUBLIC_ORIGIN='http://localhost:8080'
USER_ALLOW_LOOPBACK_HTTP='true'
USER_BIND_ADDR='127.0.0.1:8080'
USER_DATABASE_URL='sqlite://data/users.db?mode=rwc'
GOOGLE_CLIENT_ID='your-client-id.apps.googleusercontent.com'
GOOGLE_CLIENT_SECRET='your-client-secret'
```

`user-server` と Cargo は `.env` を自動では読み込みません。同じシェルで `.env` の内容を環境変数として読み込んでから起動します。

```sh
set -a
. ./.env
set +a
cargo run -p user-server
```

`.env` を変更した場合は、読み込みと起動を再実行します。

`user HTTP server listening on 127.0.0.1:8080` が表示されたら、ブラウザーで `http://localhost:8080/auth/google/start` を開きます。Google の認証後、`/me` に JSON が表示されます。初回ログインはアカウントを作成し、以後のログインは同じ内部ユーザー ID を返します。

`USER_DATABASE_URL` のファイルの親ディレクトリは起動時に作成します。ユーザー用とゲーム用には別の SQLite ファイルを指定します。

`USER_SESSION_SECONDS` でセッションの有効期間を秒単位で指定できます。省略時は `86400` です。指定できる範囲は `60` から `2592000` です。

`USER_ALLOW_LOOPBACK_HTTP` を省略すると、`USER_PUBLIC_ORIGIN` には HTTPS が必要です。`true` は `localhost`、`127.0.0.1`、`[::1]` の HTTP origin だけを許可します。開発用 HTTP で `USER_BIND_ADDR=0.0.0.0:8080` を使う場合は、コンテナーのホスト側ポートを `127.0.0.1` に限定してください。HTTP の待ち受けを信頼できないネットワークに公開しないでください。

## Docker Compose で起動する

リポジトリのルートに、上記の `.env` を用意して実行します。Compose は `.env` を自動で読み込みます。

```sh
docker compose --profile auth up --build -d user-server
```

ブラウザーで `http://localhost:8080/auth/google/start` を開きます。Compose ではコンテナー内の待ち受けアドレスと DB 保存先を `compose.yaml` で指定し、ユーザー DB を専用の `user-data` ボリュームに保存します。

## プロフィールを変更する

ログイン済みのブラウザーで、`http://localhost:8080/me` と同じ origin の開発者コンソールから実行します。

```js
await fetch('/me', {
  method: 'PATCH',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ display_name: '将棋プレイヤー' })
}).then(r => r.json())
```

表示名は前後の空白を除去した後で、1 文字から 80 文字までです。制御文字は使えません。レスポンスには内部 ID、表示名、作成時刻の Unix 秒が含まれます。

ログアウトも同じ origin から実行します。

```js
await fetch('/auth/logout', { method: 'POST' })
```

成功時は HTTP `204` です。以後の `/me` は HTTP `401` になります。表示名変更とログアウトには、`USER_PUBLIC_ORIGIN` と一致する `Origin` ヘッダーが必要です。

## 本番で起動する

1. HTTPS 終端を用意します。
2. 本番用の `.env` を用意し、`USER_PUBLIC_ORIGIN` に外部公開用の HTTPS origin を設定します。パス、クエリ、フラグメント、ユーザー情報は含めません。
3. Google の Authorized redirect URIs を本番の callback URL に合わせます。
4. `USER_ALLOW_LOOPBACK_HTTP` を削除するか `false` にします。
5. `USER_BIND_ADDR` を内部ネットワークの待ち受けアドレスに設定します。
6. 永続ボリュームにユーザー専用 SQLite ファイルを置きます。
7. 本番用の Google クライアント ID とシークレットを `.env` に設定し、ファイルの読み取り権限を実行ユーザーに限定します。
8. ローカル起動と同じ `set -a`、`. ./.env`、`set +a` の手順で読み込み、`user-server` を起動します。

TLS 終端からサービスまでの通信は信頼できる内部ネットワークに限定します。ブラウザーの origin を保ち、callback のクエリ、Cookie、OAuth レスポンスをアクセスログに記録しない設定にします。サービスは任意の `X-Forwarded-*` ヘッダーを認証判断に使いません。

起動時と認証コードの交換時には Google の discovery と JWKS を取得します。各 HTTP 要求は接続タイムアウト 5 秒、要求タイムアウト 10 秒です。リダイレクトを追わず、HTTPS のみを許可します。コード交換時の再取得で Google の鍵更新を反映します。

## 自動検証を実行する

```sh
cargo test -p user-id -p user-application -p user-persistence -p user-http -p user-server
```

テストは SQLite での初回登録の競合、再起動後のセッション、期限切れ、ログアウト後の失効、callback の再利用拒否、ブラウザーの対応関係、Origin の検証を確認します。RSA 署名付き ID トークンで issuer、audience、nonce、exp、azp、署名、access token hash の検証も確認します。HTTP の一連の操作はローカル TCP サーバーでも実行します。

実際の Google アカウントを使う認証は、上記の OAuth クライアント設定とブラウザー操作で確認します。自動テストは Google の実アカウントを使いません。
