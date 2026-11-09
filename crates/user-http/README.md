# user-http

`router` provides Google login and account routes. `GoogleProvider` uses the fixed Google issuer through `openidconnect` 4.1. It refreshes discovery and signing keys during each code exchange. Each outbound request has a timeout and cannot follow redirects.

## Routes

| Method | Path | Result |
| --- | --- | --- |
| GET | `/auth/google/start` | Creates a ten-minute browser-bound login attempt and redirects to Google. |
| GET | `/auth/google/callback` | Consumes the attempt, verifies Google identity, creates a session, and redirects to `/me`. |
| GET | `/me` | Returns the authenticated profile. |
| PATCH | `/me` | Changes the authenticated display name. Requires matching `Origin`. |
| POST | `/auth/logout` | Revokes the current session and clears the cookie. Requires matching `Origin`. |
| GET | `/openapi.json` | Returns the OpenAPI document. |

## Identity and browser sessions

The public API accepts no caller-selected internal user ID and never links identities by email. Google access and refresh tokens are discarded after token verification. Sessions use random opaque browser cookies and SHA-256 hashes in PostgreSQL, with a fixed server-side expiry. Successful login rotates the current browser session. All responses use `Cache-Control: no-store`, `Referrer-Policy: no-referrer`, and `X-Content-Type-Options: nosniff`.

Production cookies are host-only `__Host-user_session` and `__Host-user_login` cookies with `Secure`, `HttpOnly`, `SameSite=Lax`, and `Path=/`. Explicit loopback HTTP development mode uses `user_session` and `user_login` without `Secure`.

## Input validation and errors

Both `PATCH /me` and logout require `Sec-Fetch-Site: same-origin` when that
header is supplied. Logout permits a missing or invalid session cookie. Callback
query failures and JSON extractor failures use Axum plain-text responses. Rename
returns 400 for malformed JSON, 413 for an oversized body, 415 for a missing or
unsupported JSON content type, and 422 for an invalid structure. Application
errors use JSON with an `error` code, including 400 for an invalid display name.
The name is trimmed before the nonempty, 80-Unicode-character, and no-control
checks.

See the [user-server guide](../../apps/user-server/README.md) for Google client
configuration, environment variables, deployment, and a real-provider login
check. [Unfinished work](../../docs/design/unfinished-work.md) lists planned
account features and game authentication integration.

## OpenAPI

`GET /openapi.json` returns this service's OpenAPI 3.1 document. The public
`user_http::openapi()` constructor needs no database, service configuration, or
provider connection. Handler annotations, serde DTOs, and one `OpenApiRouter`
registry define both the runtime routes and the document. Register new handlers
with `routes!` in `api_router()` and keep their `#[utoipa::path]` annotations
next to the handlers. No separate document path list is maintained.

Export this service's document without starting the server.

```sh
cargo run --locked -p user-http --example user-openapi
```

The [root guide](../../README.md#generate-api-documentation) describes exporting
both HTTP documents. Generated JSON is a build artifact.

## Verification

Run from the repository root.

```sh
./scripts/test-postgres.sh cargo test --locked -p user-http
```

Tests use a simulated identity provider and check the generated OpenAPI
contract. [Rust CI](../../.github/workflows/rust.yaml) runs them with the
workspace tests.
