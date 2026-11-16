use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Pure scalar storage-boundary cases.",
        reason: "Direct scalar conversion assertions reject values outside PostgreSQL signed-integer storage bounds.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
    fn record_scalars_reject_values_outside_storage_bounds() {
        for value in [0u64, 1, i64::MAX as u64] {
            assert_eq!(OperationSequence::try_from(value).unwrap().get(), value);
            assert_eq!(MovePly::try_from(value).unwrap().get(), value);
        }
        for value in [i64::MAX as u64 + 1, u64::MAX] {
            assert_eq!(OperationSequence::try_from(value), Err(Error::Corrupt));
            assert_eq!(MovePly::try_from(value), Err(Error::Corrupt));
        }
        for value in [i64::MIN, -1] {
            assert_eq!(OperationSequence::try_from(value), Err(Error::Corrupt));
            assert_eq!(MovePly::try_from(value), Err(Error::Corrupt));
            assert_eq!(PreparedFinishTime::try_from(value), Err(Error::Corrupt));
        }
        for value in [0i64, 1, i64::MAX] {
            assert_eq!(
                OperationSequence::try_from(value).unwrap().get(),
                value as u64
            );
            assert_eq!(MovePly::try_from(value).unwrap().get(), value as u64);
            assert_eq!(PreparedFinishTime::try_from(value).unwrap().get(), value);
        }
    }
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Pure metadata constructor cases without a database or legal game replay.",
        reason: "Direct constructors reject listed duplicate-player, zero-sequence and inconsistent move/resignation count cases.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
    fn metadata_construction_requires_distinct_players_and_consistent_finish_counts() {
        for (black, white, reason, seq, ply, valid) in [
            ("black", "white", FinishReason::Checkmate, 1u64, 1u64, true),
            ("black", "white", FinishReason::Resignation, 1, 0, true),
            ("black", "white", FinishReason::Resignation, 3, 2, true),
            ("black", "white", FinishReason::Checkmate, 0, 0, false),
            ("black", "white", FinishReason::Resignation, 0, 0, false),
            ("black", "black", FinishReason::Resignation, 1, 0, false),
            ("black", "white", FinishReason::Resignation, 1, 1, false),
            ("black", "white", FinishReason::Checkmate, 1, 0, false),
            ("black", "white", FinishReason::Checkmate, 1, 2, false),
            (
                "black",
                "white",
                FinishReason::Resignation,
                i64::MAX as u64,
                i64::MAX as u64 - 1,
                true,
            ),
            (
                "black",
                "white",
                FinishReason::Checkmate,
                i64::MAX as u64,
                i64::MAX as u64,
                true,
            ),
        ] {
            let metadata = Metadata::new(
                GameId::new("id").unwrap(),
                UserId::new(black).unwrap(),
                UserId::new(white).unwrap(),
                Rule::Standard,
                GameResult {
                    winner: Player::Black,
                    reason,
                },
                PreparedFinishTime::try_from(0).unwrap(),
                OperationSequence::try_from(seq).unwrap(),
                MovePly::try_from(ply).unwrap(),
            );
            assert_eq!(
                metadata.is_ok(),
                valid,
                "{black}, {white}, {reason:?}, {seq}, {ply}"
            );
            if let Ok(metadata) = metadata {
                assert_eq!(metadata.id().as_str(), "id");
                assert_eq!(metadata.black().as_str(), black);
                assert_eq!(metadata.white().as_str(), white);
                assert_eq!(metadata.rule(), Rule::Standard);
                assert_eq!(
                    metadata.result(),
                    GameResult {
                        winner: Player::Black,
                        reason
                    }
                );
                assert_eq!(metadata.finished_at().get(), 0);
                assert_eq!(metadata.final_seq().get(), seq);
                assert_eq!(metadata.final_move_ply().get(), ply);
            } else {
                assert_eq!(metadata, Err(Error::Corrupt));
            }
        }
    }
}
