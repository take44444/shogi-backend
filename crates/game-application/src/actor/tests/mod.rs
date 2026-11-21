#![cfg(test)]

use super::*;
use crate::{
    CommittedEvent, DomainEvent, GameService, MAILBOX_CAPACITY, MakeMove, UserId, ViewMetadata,
    ViewerGameRecord, Visibility,
};
use async_trait::async_trait;
use std::sync::Mutex;

mod support;
use support::{id, receipt, store};

mod command_application;
mod mailbox;
mod registration;
mod sequence_limits;
