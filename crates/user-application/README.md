# user-application

Account types and the asynchronous `AccountRepository` port. Profiles contain a
shared `UserId`, a `DisplayName`, and a creation time. External identities use a
verified issuer and subject. Login attempts and sessions carry expiry times.

## Validation and responsibilities

`DisplayName::new` trims surrounding whitespace. It rejects empty names, names
over 80 Unicode characters, and names containing control characters.
`LoginAttempt`, `VerifiedIdentity`, and `NewSession` have no `Debug`
implementation because they carry authentication data.

The [HTTP adapter](../user-http/README.md) verifies Google identity before
invoking the repository. The repository must consume login attempts atomically
and resolve an account and create its session in one transaction. The
[PostgreSQL adapter](../user-persistence/README.md) implements those operations.
This crate has no game, HTTP, database, or OAuth library dependency.

## Verification

Run display-name validation tests from the repository root.

```sh
cargo test --locked -p user-application
```

Repository transaction and expiry behavior is tested in `user-persistence`.
