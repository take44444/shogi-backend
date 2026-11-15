crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Pure Elo equal-player example, no persistence.",
        reason: "Equal unrated players become 1516 and 1484 with the expected completed counts.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
    fn equal_players_gain_and_lose_sixteen() {
        let c = calculate(RatingState::Unrated, RatingState::Unrated, Player::Black).unwrap();
        assert_eq!(c.black.rating(), Some(1516));
        assert_eq!(c.white.rating(), Some(1484));
        assert_eq!(c.black.completed(), 1);
        assert_eq!(c.white.completed(), 1);
        assert_eq!(
            calculate(RatingState::Unrated, RatingState::Unrated, Player::White)
                .unwrap()
                .delta_black,
            -16
        );
    }
}
