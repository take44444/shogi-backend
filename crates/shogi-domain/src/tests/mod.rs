#![cfg(test)]

use super::*;

mod support;
use support::{fixture, game_from_rows, mv, play, reject, snapshot, sq};

mod legal_moves;
mod notation;
mod play;
mod position;
mod resignation;
