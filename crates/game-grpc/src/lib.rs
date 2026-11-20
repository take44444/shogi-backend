//! gRPC transport for durable shogi commands, with generated [`proto`] clients and servers.
//!
//! [`GameRpc`] adapts [`game_application::GameService`] to `shogi.v1.GameService`.
//! The schema lives in `proto/game.proto`; vendored protoc generates Rust at build
//! time. The supplied server uses plaintext gRPC. User IDs are supplied claims,
//! not authentication credentials. Deploy this unauthenticated listener for trusted
//! internal callers. The separate HTTP user service does not authenticate gRPC calls.
//!
//! Move and resignation requests require `expected_seq`. Protobuf optional presence distinguishes
//! omission from valid zero. IDs contain 1 through 128 UTF-8 bytes without Unicode
//! whitespace, and moves use strict USI syntax. Exact accepted move and resignation retries return
//! the original acceptance. After a lost response or `Unavailable`, retry the same
//! ID and identical contents. Cancellation after enqueue does not cancel execution.
//!
//! # Status mapping
//!
//! | Outcome | gRPC code |
//! | --- | --- |
//! | Malformed ID, move, missing sequence, invalid rating pool, or equal participants | `InvalidArgument` |
//! | Unknown game | `NotFound` |
//! | Nonparticipant | `PermissionDenied` |
//! | Duplicate game, or changed accepted request | `AlreadyExists` |
//! | Stale sequence | `Aborted`, with `actual-seq` metadata |
//! | Wrong turn, illegal move, finished game, or occupied rating pool | `FailedPrecondition` |
//! | Storage failure or uncertain commit | `Unavailable` |
//! | Corrupt history or incompatible schema | `DataLoss` |
//! | Internal invariant violation, rules failure, or sequence exhaustion | `Internal` |
//!
#![warn(missing_docs)]

mod conversion;
mod service;

/// Messages, enums, and Tonic client and server bindings generated from `proto/game.proto`.
/// Schema comments define field contracts; zero enum values are wire defaults,
/// not valid domain players or pieces. The facade emits concrete enum values.
pub mod proto {
    tonic::include_proto!("shogi.v1");
}

pub use service::GameRpc;
