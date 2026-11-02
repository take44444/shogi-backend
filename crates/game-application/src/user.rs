//! User registration and lookup through the user repository.

use crate::{ApplicationError, User, UserId, UserRepository};
use std::sync::Arc;

/// Cloneable user application service backed by a user repository.
/// User IDs are caller claims, not authentication credentials.
#[derive(Clone)]
pub struct UserService {
    repository: Arc<dyn UserRepository>,
}

impl UserService {
    /// Creates a service with the supplied user repository.
    /// Construction performs no database access.
    pub fn new(repository: Arc<dyn UserRepository>) -> Self {
        Self { repository }
    }

    /// Creates a user with the supplied internal ID. This does not authenticate anyone.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::AlreadyExists`] for a duplicate ID, or
    /// [`ApplicationError::Unavailable`] on a storage failure.
    pub async fn create_user(&self, id: UserId) -> Result<User, ApplicationError> {
        self.repository.create_user(id).await
    }

    /// Loads an existing user.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::UserNotFound`] for an unknown user, or
    /// [`ApplicationError::Unavailable`] on a storage failure.
    pub async fn get_user(&self, id: &UserId) -> Result<User, ApplicationError> {
        self.repository.get_user(id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::{collections::HashSet, sync::Mutex};

    #[derive(Default)]
    struct UsersOnly(Mutex<HashSet<UserId>>);

    #[async_trait]
    impl UserRepository for UsersOnly {
        async fn create_user(&self, id: UserId) -> Result<User, ApplicationError> {
            if !self.0.lock().unwrap().insert(id.clone()) {
                return Err(ApplicationError::AlreadyExists);
            }
            Ok(User { id })
        }

        async fn get_user(&self, id: &UserId) -> Result<User, ApplicationError> {
            if !self.0.lock().unwrap().contains(id) {
                return Err(ApplicationError::UserNotFound);
            }
            Ok(User { id: id.clone() })
        }
    }

    #[tokio::test]
    async fn registration_and_lookup_need_only_a_user_repository() {
        let users = UserService::new(Arc::new(UsersOnly::default()));
        let other = users.clone();
        let id = UserId::new("registered").unwrap();
        assert_eq!(
            users.get_user(&id).await,
            Err(ApplicationError::UserNotFound)
        );
        let created = users.create_user(id.clone()).await.unwrap();
        assert_eq!(created.id, id);
        assert_eq!(other.get_user(&id).await, Ok(created));
        assert_eq!(
            other.create_user(id).await,
            Err(ApplicationError::AlreadyExists)
        );
    }
}
