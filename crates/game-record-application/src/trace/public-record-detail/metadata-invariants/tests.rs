use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "保存層のスカラー値の境界を直接確認する。",
        reason: "スカラー値の変換を直接検証し、PostgreSQL の符号付き整数として保存できる範囲外の値を拒否することを確認する。",
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
        scope: "データベースや合法な対局の再生を使わず、メタデータの構築を直接確認する。",
        reason: "列挙した、参加者の重複、シーケンス番号 0、着手・投了時の手数と操作数の不整合を、直接の構築で拒否することを確認する。",
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
