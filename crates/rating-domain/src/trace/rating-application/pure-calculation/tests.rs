use super::*;
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

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Pure rating-state/calculation examples and arithmetic boundaries.",
        reason: "Listed starts/winners cover an upset, zero-sum deltas within 32, checked extreme arithmetic and invalid rating-state combinations.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "user-ratings", contract: "rating-state", scope: "RatingState の不正な組み合わせ 2 例と計算の境界を直接確認する。DB と HTTP は通らない。", relationship: crate::trace_support::Relationship::Asserts, reason: "Some(1500) と対局数 0、および None と対局数 1 の組み合わせを拒否することを確認する。" }],
    };
#[test]
    fn expected_upsets_and_extremes_are_zero_sum_and_checked() {
        let state = |n| RatingState::from_parts(Some(n), 3).unwrap();
        for (b, w) in [(1500, 1900), (1900, 1500), (i64::MIN + 32, i64::MAX - 32)] {
            for winner in [Player::Black, Player::White] {
                let c = calculate(state(b), state(w), winner).unwrap();
                assert_eq!(
                    c.black.effective() as i128 + c.white.effective() as i128,
                    b as i128 + w as i128
                );
                assert!(c.delta_black.abs() <= 32);
            }
        }
        assert_eq!(
            calculate(state(1500), state(1900), Player::Black)
                .unwrap()
                .delta_black,
            29
        );
        assert!(calculate(state(i64::MAX), state(i64::MAX), Player::Black).is_err());
        assert!(
            calculate(
                RatingState::from_parts(Some(1500), i64::MAX).unwrap(),
                state(1500),
                Player::Black
            )
            .is_err()
        );
        assert!(RatingState::from_parts(Some(1500), 0).is_err());
        assert!(RatingState::from_parts(None, 1).is_err());
    }
}
