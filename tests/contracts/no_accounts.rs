async fn register(repository: &dyn game_application::GameRepository, user: game_application::UserId) {
    repository.create_user(user).await;
}
