#![cfg(test)]

use game_record_application::ratings::{UserRatingSnapshot, UserRatings};
use game_record_application::{Error, UserId};
use std::sync::Arc;

pub(crate) fn unused_ratings() -> UserRatings {
    struct Unused;
    #[async_trait::async_trait]
    impl game_record_application::ratings::RatingRepository for Unused {
        async fn ratings(&self, _: &UserId) -> Result<UserRatingSnapshot, Error> {
            panic!("route must not query ratings")
        }
    }
    UserRatings::new(Arc::new(Unused))
}
