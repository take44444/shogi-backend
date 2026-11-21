use super::*;

#[test]
fn ids_validate_bytes_and_unicode_whitespace() {
    for invalid in [
        "".to_owned(),
        " ".into(),
        "x\ty".into(),
        "x\ny".into(),
        "x\u{2003}y".into(),
        "x".repeat(129),
        "é".repeat(65),
    ] {
        assert!(GameId::new(invalid.clone()).is_err());
        assert!(RequestId::new(invalid).is_err());
    }
    for valid in ["game-1".to_owned(), "x".repeat(128), "é".repeat(64)] {
        assert_eq!(GameId::new(valid.clone()).unwrap().as_str(), valid);
        assert_eq!(RequestId::new(valid.clone()).unwrap().as_str(), valid);
    }
}
