use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "受理記録からイベントを作る外部依存のない処理を検証し、PostgreSQL や通信は使わない。",
        reason: "正規の着手・詰み・投了のイベントを直接比較し、矛盾する操作と結果の組み合わせを拒否する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "game-command-recovery", contract: "canonical-events", scope: "合成した受理記録から StagedGameChange を直接構築する。詰みに至る実際の対局やデータベースは使わない。", relationship: crate::trace_support::Relationship::Asserts, reason: "通常の着手、同一シーケンスでの着手と詰みによる終局、投了の各イベント列を完全に比較する。矛盾する操作と結果は拒否する。" }],
    };
#[test]
    fn canonical_move_mate_and_resignation_events() {
        let movement = "7g7f".parse().unwrap();
        let event = |event| CommittedEvent {
            game_id: game(),
            request_id: RequestId::new("request").unwrap(),
            user_id: UserId::new("black").unwrap(),
            seq: 1,
            event,
        };
        let normal = StagedGameChange::new(game(), receipt(Action::Move(movement), None)).unwrap();
        assert_eq!(
            normal.events(),
            &[event(DomainEvent::MoveCommitted {
                movement,
                in_check: false,
                result: None,
            })]
        );
        let mate = GameResult {
            winner: Player::Black,
            reason: FinishReason::Checkmate,
        };
        let terminal =
            StagedGameChange::new(game(), receipt(Action::Move(movement), Some(mate))).unwrap();
        assert_eq!(
            terminal.events(),
            &[
                event(DomainEvent::MoveCommitted {
                    movement,
                    in_check: true,
                    result: Some(mate),
                }),
                event(DomainEvent::GameFinished { result: mate }),
            ]
        );
        let result = GameResult {
            reason: FinishReason::Resignation,
            ..mate
        };
        let end = StagedGameChange::new(game(), receipt(Action::Resign, Some(result))).unwrap();
        assert_eq!(end.events(), &[event(DomainEvent::GameFinished { result })]);
        for bad in [
            receipt(Action::Resign, None),
            receipt(Action::Resign, Some(mate)),
            receipt(Action::Move(movement), Some(result)),
        ] {
            assert_eq!(
                StagedGameChange::new(game(), bad),
                Err(ApplicationError::Corrupt)
            );
        }
    }
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "外部依存のない、ステージ済みシーケンスの境界と連続性の事例を検証する。",
        reason: "受理シーケンスを直接与え、連続性より先に保存範囲の上限を確認し、連続しないシーケンスを拒否する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "game-command-recovery", contract: "canonical-events", scope: "合成した受理記録でシーケンス境界を直接検証する。", relationship: crate::trace_support::Relationship::Asserts, reason: "0・不連続・最大値・範囲外の受理シーケンスと、予想シーケンスの境界を確認する。" }],
    };
#[test]
    fn staging_checks_sequence_bounds_before_continuity() {
        let limit = i64::MAX as u64;
        for (expected, accepted, error) in [
            (limit, limit, ApplicationError::SequenceExhausted),
            (limit, limit + 1, ApplicationError::SequenceExhausted),
            (u64::MAX, 0, ApplicationError::SequenceExhausted),
            (0, limit + 1, ApplicationError::SequenceExhausted),
            (0, u64::MAX, ApplicationError::SequenceExhausted),
            (0, 0, ApplicationError::Corrupt),
            (0, 2, ApplicationError::Corrupt),
            (1, 1, ApplicationError::Corrupt),
        ] {
            let mut receipt = receipt(Action::Move("7g7f".parse().unwrap()), None);
            receipt.request.expected_seq = expected;
            receipt.acceptance.seq = accepted;
            assert_eq!(StagedGameChange::new(game(), receipt), Err(error));
        }
        let mut receipt = receipt(Action::Move("7g7f".parse().unwrap()), None);
        receipt.request.expected_seq = limit - 1;
        receipt.acceptance.seq = limit;
        let change = StagedGameChange::new(game(), receipt.clone()).unwrap();
        assert_eq!(change.game_id(), &game());
        assert_eq!(change.receipt(), &receipt);
        assert_eq!(change.events()[0].seq, limit);
    }
}
