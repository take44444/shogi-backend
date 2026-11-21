use super::*;

#[path = "../support/public_record_detail.rs"]
mod public_record_detail_support;
use public_record_detail_support::{
    Failing, MakeMove, Participants, PostgresGameRecords, RequestId, Resign, SeaOrmStore, WriteId,
    get,
};

mod application_forwarding;
mod responses;
