# User ID

`UserId` is a shared internal identifier, never an authentication credential. `UserId::new` accepts 1 through 128 UTF-8 bytes with no Unicode whitespace. It preserves the original string. `as_str` borrows that string. Invalid inputs return `InvalidId`.
