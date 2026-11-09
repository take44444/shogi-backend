# user-id

`UserId` is an internal identifier shared by game and account code. Possessing
an ID does not authenticate its caller.

`UserId::new` accepts 1 through 128 UTF-8 bytes with no Unicode whitespace. The
limit counts bytes, not characters. It preserves the original string, which
`as_str` borrows. Invalid input returns `InvalidId`.

Run validation tests from the repository root.

```sh
cargo test --locked -p user-id
```
