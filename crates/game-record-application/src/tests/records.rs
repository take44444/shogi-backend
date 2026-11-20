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
#[path = "records_metadata.rs"]
mod public_record_detail_metadata_invariants;

#[path = "records_search.rs"]
mod public_record_search_bounds_cursor;
