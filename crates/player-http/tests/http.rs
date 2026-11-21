use axum::{
    Router,
    http::{StatusCode, header},
};
use game_record_application::ratings::UserRatings;
use game_record_application::{Detail, Error, GameId, GameRecords, Page, Search, UserId};
use player_http::{openapi, router};
use std::sync::Arc;

#[path = "support/router.rs"]
mod record_route_support;

mod public_record_detail;
mod user_ratings;
