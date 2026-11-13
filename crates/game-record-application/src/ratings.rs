//! Latest committed ratings, independent of account existence and pending work.
use crate::{Error, UserId};
use async_trait::async_trait;
pub use rating_domain::{Pool, RatingState};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct UserRatingSnapshot {
    pub ten_minute_30: RatingState,
    pub byoyomi_30: RatingState,
}
#[async_trait]
pub trait RatingRepository: Send + Sync {
    async fn ratings(&self, user: &UserId) -> Result<UserRatingSnapshot, Error>;
}
#[derive(Clone)]
pub struct UserRatings {
    repository: Arc<dyn RatingRepository>,
}
impl UserRatings {
    pub fn new(repository: Arc<dyn RatingRepository>) -> Self {
        Self { repository }
    }
    pub async fn get(&self, user: &UserId) -> Result<UserRatingSnapshot, Error> {
        self.repository.ratings(user).await
    }
}
