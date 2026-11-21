use super::*;

pub(super) fn filters() -> Filters {
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
