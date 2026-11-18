use super::*;

#[test]
fn identifier_codec_preserves_storage_escapes_and_rejects_malformed_values() {
    for (raw, stored) in [
        ("plain", "plain"),
        ("a\0b", "a\u{1}\u{1}b"),
        ("a\u{1}b", "a\u{1}\u{2}b"),
        ("\0\u{1}\0", "\u{1}\u{1}\u{1}\u{2}\u{1}\u{1}"),
    ] {
        assert_eq!(encode_id(raw), stored);
        assert_eq!(decode(stored.to_owned()).unwrap(), raw);
    }
    for malformed in ["\u{1}", "a\u{1}x", "a\u{1}\u{2}\u{1}"] {
        assert_eq!(decode(malformed.to_owned()), Err(Error::Corrupt));
    }
}
