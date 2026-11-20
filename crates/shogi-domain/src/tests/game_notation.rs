use super::*;

#[test]
fn square_bounds_and_notation_are_strict() {
    for file in 0..=u8::MAX {
        for rank in 0..=u8::MAX {
            assert_eq!(
                Square::new(file, rank).is_ok(),
                (1..=9).contains(&file) && (1..=9).contains(&rank)
            );
        }
    }
    for file in 1..=9 {
        for rank in 1..=9 {
            let square = Square::new(file, rank).unwrap();
            assert_eq!(square.file(), file);
            assert_eq!(square.rank(), rank);
            assert_eq!(square.to_string().parse::<Square>(), Ok(square));
        }
    }
    for valid in [
        "7g7f", "2b3c+", "P*5e", "L*1b", "N*1c", "S*3a", "G*4i", "B*8h", "R*2b",
    ] {
        assert_eq!(mv(valid).to_string(), valid);
    }
    for invalid in [
        "", " 7g7f", "7g7f\n", "7g7f++", "7g7f-", "7G7f", "0g7f", "7j7f", "7g0f", "7g7j", "P*5e+",
        "p*5e", "K*5e", "+P*5e", "resign", "win", "0000", "7g7f\0", "é*5e", "７g7f", "7g7🦀",
    ] {
        assert!(invalid.parse::<Move>().is_err(), "{invalid:?}");
    }
    for invalid in ["", "1A", "0a", "1j", "1aa", " 1a", "１a", "🦀"] {
        assert!(invalid.parse::<Square>().is_err());
    }
}
