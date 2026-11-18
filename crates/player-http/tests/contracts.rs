use axum::{
    Router,
    http::{StatusCode, header},
};
use game_record_application::ratings::{UserRatingSnapshot, UserRatings};
use game_record_application::{Detail, Error, GameId, GameRecords, Page, Search, UserId};
use player_http::{openapi, router};
use std::sync::Arc;

#[path = "contracts/user-ratings/support.rs"]
mod rating_tests;
#[path = "../src/tests/public-record-detail/router_support.rs"]
mod record_route_support;
#[path = "contracts/public-record-detail/support.rs"]
mod tests;
#[path = "../../../scripts/traceability_model.rs"]
mod trace_support;
