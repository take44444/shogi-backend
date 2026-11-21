fn change(players: &mut game_application::Participants, user: game_application::UserId) {
    players.black = user;
}
