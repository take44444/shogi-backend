#[path = "support/server.rs"]
mod server_support;
use server_support::{
    Arc, Code, GameService, SeaOrmStore, SeedGame, TestServer, command, create, hand, piece, proto,
    resign, snapshot,
};

mod game_commands;
mod game_play;
#[path = "matchmaking/cancellation.rs"]
mod matchmaking;
