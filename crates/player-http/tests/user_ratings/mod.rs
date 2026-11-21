use super::*;

#[path = "../support/user_ratings.rs"]
mod user_ratings_support;
use user_ratings_support::{Body, NoRecords, RatingState, Request, UserRatingSnapshot, get};

mod application_forwarding;
mod input_validation;
mod openapi;

use user_ratings_support::ServiceExt;
