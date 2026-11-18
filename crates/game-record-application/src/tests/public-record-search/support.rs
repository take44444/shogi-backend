#![cfg(test)]

use super::*;
fn filters() -> Filters {
    Filters {
        participant: None,
        rule: None,
        winner: None,
        reason: None,
        from: 0,
        to: 100,
        min_ply: None,
        max_ply: None,
    }
}
#[path = "../public-record-detail/metadata-invariants/tests.rs"]
mod public_record_detail_metadata_invariants;

#[path = "bounds-cursor/tests.rs"]
mod public_record_search_bounds_cursor;
