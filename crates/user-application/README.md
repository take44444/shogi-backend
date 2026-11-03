# User application

Account domain types and the asynchronous `AccountRepository` boundary. Profiles have a shared validated `UserId`, a validated display name, and a creation time. External identity uses the verified issuer and subject. Login attempts and sessions carry explicit expiry times.

This crate has no game, HTTP, database, or OAuth library dependency. Authentication credentials have no `Debug` implementation. The HTTP adapter verifies Google identity before invoking the repository. The persistence adapter makes account resolution and session creation atomic.
