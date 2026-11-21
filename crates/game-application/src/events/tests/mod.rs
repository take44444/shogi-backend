#![cfg(test)]

use super::*;
use crate::Acceptance;
use crate::MutationRequest;
use shogi_domain::Player;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

mod support;
use support::{Queue, Sink, game, receipt};

mod canonical_facts;
mod confirmed_dispatch;
