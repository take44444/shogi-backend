#![doc = include_str!("../README.md")]

mod conversion;
mod service;

pub mod proto {
    tonic::include_proto!("shogi.v1");
}

pub use service::GameRpc;
