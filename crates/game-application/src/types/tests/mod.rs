use super::*;

#[test]
fn participant_bindings_are_distinct_and_preserve_sides() {
    let black = UserId::new("black").unwrap();
    let white = UserId::new("white").unwrap();
    assert_eq!(
        Participants::new(black.clone(), black.clone()),
        Err(ApplicationError::InvalidParticipants)
    );
    let participants = Participants::new(black.clone(), white.clone()).unwrap();
    assert_eq!(participants.black(), &black);
    assert_eq!(participants.white(), &white);
    assert_eq!(participants.side(&black), Ok(Player::Black));
    assert_eq!(participants.side(&white), Ok(Player::White));
    assert_eq!(
        participants.side(&UserId::new("outsider").unwrap()),
        Err(ApplicationError::NonParticipant)
    );
}

mod identifiers;
