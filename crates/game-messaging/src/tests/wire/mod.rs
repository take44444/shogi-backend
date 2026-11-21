#![cfg(test)]

use crate::*;
use game_application::{RequestId, UserId};
use serde_json::json;

mod support;
use support::event;

mod event_frame;
mod format;
