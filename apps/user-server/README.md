# User server

The standalone HTTP account service composes `user-http`, `user-application`, and `user-persistence`. It runs independently of `game-server` and uses a separate SQLite database.

Run `cargo run -p user-server` after following [the Google login setup guide](../../docs/user-service.md).
