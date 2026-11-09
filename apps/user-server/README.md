# User server

The standalone HTTP account service composes `user-http`, `user-application`, and `user-persistence`. It runs independently of `game-server` and uses a separate PostgreSQL database.

The [authentication decision](../../docs/adr/0002-browser-authentication.md) explains authentication choices. See [unfinished work](../../docs/design/unfinished-work.md) for remaining features and verification limits. The [HTTP API reference](../../crates/user-http/README.md) describes the routes.

## Configuration

Create a Google OAuth client of type **Web application**. Its authorized redirect URI must exactly match `USER_PUBLIC_ORIGIN` followed by `/auth/google/callback`. For local development, use `http://localhost:8080/auth/google/callback`. If the Google app is in Testing, add the accounts used for development as test users. The service requests `openid profile`. See [Google OpenID Connect](https://developers.google.com/identity/openid-connect/openid-connect) for provider configuration.

| Variable | Required | Default and constraints |
| --- | --- | --- |
| `GOOGLE_CLIENT_ID` | Yes | Google web client ID. |
| `GOOGLE_CLIENT_SECRET` | Yes | Google web client secret. |
| `USER_PUBLIC_ORIGIN` | Yes | HTTPS origin without credentials, path, query, or fragment. Determines the callback URL and allowed mutation Origin. |
| `USER_DATABASE_URL` | Yes | PostgreSQL URL for the separate `users` database. The database must already exist. |
| `USER_BIND_ADDR` | No | `127.0.0.1:8080`. Internal HTTP listener address. |
| `USER_ALLOW_LOOPBACK_HTTP` | No | `false`. Set `true` to allow an HTTP public origin on `localhost`, `127.0.0.1`, or `[::1]`. |
| `USER_SESSION_SECONDS` | No | `86400`. Absolute session lifetime, from `60` to `2592000` seconds. |

## Run locally

Run commands from the repository root. Start PostgreSQL with `docker compose up -d --wait postgres`. Copy the configuration template unless `.env` already exists.

```sh
cp .env.example .env
```

Set the Google credentials in `.env`. A local configuration is:

```dotenv
GOOGLE_CLIENT_ID='your-client-id.apps.googleusercontent.com'
GOOGLE_CLIENT_SECRET='your-client-secret'
USER_PUBLIC_ORIGIN='http://localhost:8080'
USER_ALLOW_LOOPBACK_HTTP='true'
USER_BIND_ADDR='127.0.0.1:8080'
USER_DATABASE_URL='postgres://shogi:shogi@localhost:5432/users'
```

The application and Cargo do not load `.env` automatically. Export its values in the current shell before starting the server. Reload the file and restart the process after changing configuration.

```sh
set -a
. ./.env
set +a
cargo run -p user-server
```

Startup reports `user HTTP server listening on 127.0.0.1:8080`. Open `http://localhost:8080/auth/google/start` to sign in. A successful callback redirects to `/me`, which returns the authenticated profile as JSON. First login creates an account; later logins resolve the same internal user ID.

## Run with Docker Compose

Compose reads the repository-root `.env` for Google credentials and the public origin.

```sh
docker compose --profile auth up --build -d user-server
```

The `auth` profile is optional. The default Compose profile runs the game services. For the user service, Compose sets the container listener to `0.0.0.0:8080`, publishes the port on host loopback, and connects to the separate `users` database in PostgreSQL. The PostgreSQL service retains its data in `pg-data`. Container listener and database settings come from [compose.yaml](../../compose.yaml), rather than those entries in `.env`.

## Deployment

Set a public HTTPS origin and register its exact callback URL with Google. Disable loopback HTTP development mode. Terminate TLS at a reverse proxy and restrict access to the internal HTTP listener. Preserve the browser's Origin header. The service does not use `X-Forwarded-*` headers for authentication decisions.

Store the user database on persistent storage. Keep `.env` and Google credentials out of version control and restrict file access to the service operator. Exclude callback query strings, Cookies, and OAuth responses from proxy access logs. Do not expose a development HTTP listener to an untrusted network.

Startup requires Google discovery and JWKS access before the listener opens. Code exchange retrieves them again to reflect key rotation. Outbound requests use HTTPS, disable redirects, and have a five-second connection timeout and a ten-second timeout per request.

## Verification

Run the account-service tests from the repository root.

```sh
./scripts/test-postgres.sh cargo test --locked -p user-id -p user-application -p user-persistence -p user-http -p user-server
```

Tests cover signed ID-token validation, PKCE code exchange, browser-bound login attempts, concurrent registration, profile updates, CSRF checks, session expiry, restart, and logout. Local HTTP tests use a simulated provider; real Google login requires a configured OAuth client and browser interaction.

`GET /openapi.json` serves the OpenAPI 3.1 contract. To export it without server
startup or configuration, run `cargo run -p user-http --example user-openapi`.
See the [HTTP adapter reference](../../crates/user-http/README.md) for generation
and contract checks.
