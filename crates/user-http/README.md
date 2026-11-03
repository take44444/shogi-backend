# User HTTP service

`router` provides Google login and account routes. `GoogleProvider` uses the fixed Google issuer through `openidconnect` 4.1. It refreshes discovery and signing keys during each code exchange. Each outbound request has a timeout and cannot follow redirects.

| Method | Path | Result |
| --- | --- | --- |
| GET | `/auth/google/start` | Creates a ten-minute browser-bound login attempt and redirects to Google. |
| GET | `/auth/google/callback` | Consumes the attempt, verifies Google identity, creates a session, and redirects to `/me`. |
| GET | `/me` | Returns the authenticated profile. |
| PATCH | `/me` | Changes the authenticated display name. Requires matching `Origin`. |
| POST | `/auth/logout` | Revokes the current session and clears the cookie. Requires matching `Origin`. |

The public API accepts no caller-selected internal user ID and never links identities by email. Google access and refresh tokens are discarded after token verification. Sessions use random opaque browser cookies and SHA-256 hashes in SQLite, with a fixed server-side expiry. Successful login rotates the current browser session. All responses use `Cache-Control: no-store` and `Referrer-Policy: no-referrer`.

Production cookies are host-only `__Host-user_session` and `__Host-user_login` cookies with `Secure`, `HttpOnly`, `SameSite=Lax`, and `Path=/`. Explicit loopback HTTP development mode uses `user_session` and `user_login` without `Secure`.

The account service does not expose user gRPC, WebSocket, presence, matchmaking, routing, or gateway endpoints. Existing game records remain unchanged. Account deletion and administrative account management are outside this implementation.

See the [user-server README](../../apps/user-server/README.md) for Google client configuration, environment variables, and startup.
