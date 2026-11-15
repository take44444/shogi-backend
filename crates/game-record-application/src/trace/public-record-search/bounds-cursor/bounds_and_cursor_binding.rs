crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Pure Search/filter/cursor value checks without HTTP or PostgreSQL.",
        reason: "Direct cases reject bad bounds and malformed cursors, accept changed limit, preserve cursor bytes, and reject changed participant.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
    fn bounds_and_cursor_binding() {
        for (from, to, min, max, limit) in [
            (0, 0, None, None, 30),
            (0, MAX_PERIOD_MICROS + 1, None, None, 30),
            (-1, 1, None, None, 30),
            (0, 100, Some(2), Some(1), 30),
            (0, 100, None, None, 101),
        ] {
            let mut f = filters();
            f.from = from;
            f.to = to;
            f.min_ply = min;
            f.max_ply = max;
            assert!(Search::new(f, limit, None).is_err());
        }
        let search = Search::new(filters(), 1, None).unwrap();
        let metadata = Metadata::new(
            GameId::new("id").unwrap(),
            UserId::new("black").unwrap(),
            UserId::new("white").unwrap(),
            Rule::Standard,
            GameResult {
                winner: Player::Black,
                reason: FinishReason::Resignation,
            },
            PreparedFinishTime::try_from(20).unwrap(),
            OperationSequence::try_from(1u64).unwrap(),
            MovePly::try_from(0u64).unwrap(),
        )
        .unwrap();
        let cursor = search.cursor(&metadata);
        assert_eq!(
            cursor,
            "WzEsMjAsImlkIiwiOFNDYU54TWdkOElJOVlnbVpKZ2gzZWctd0xmMTM5YW9XdkYzRWZ1bjJNayJd"
        );
        let decoded = Search::new(filters(), 100, Some(&cursor)).unwrap();
        let seek = decoded.seek().unwrap();
        assert_eq!(seek.finished_at.get(), 20);
        assert_eq!(seek.id.as_str(), "id");
        assert!(Search::new(filters(), 100, Some(&cursor)).is_ok());
        let mut changed = filters();
        changed.participant = Some(UserId::new("black").unwrap());
        assert!(Search::new(changed, 1, Some(&cursor)).is_err());
        assert!(Search::new(filters(), 1, Some("garbage")).is_err());
    }
}
