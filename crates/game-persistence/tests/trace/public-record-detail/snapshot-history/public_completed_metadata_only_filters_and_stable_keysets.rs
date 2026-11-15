crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL public-record filtering and hidden-detail lookup.",
        reason: "Repository lookup directly asserts a hidden completed record is not found.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "public-record-search", contract: "bounds-cursor", scope: "Real PostgreSQL reader and writer fixtures exercise filters, visibility and keyset changes.", relationship: crate::trace_support::Relationship::Asserts, reason: "A repository page cursor is rejected after changing the winner filter." }, crate::trace_support::Evidence { feature: "public-record-search", contract: "public-keysets", scope: "Real PostgreSQL reader and writer fixtures exercise filters, visibility and keyset changes.", relationship: crate::trace_support::Relationship::Asserts, reason: "Direct repository assertions cover individual filters, a combined case, c/b ID tie ordering followed by older a, exclusion of an unseen newer row, and omission of a newly hidden row." }],
    };
#[tokio::test]
async fn public_completed_metadata_only_filters_and_stable_keysets() {
    let f = Fixture::new().await;
    f.completed("a", "alice", "bob", 0, "alice", 10).await;
    f.completed("b", "carol", "alice", 1, "alice", 20).await;
    f.completed("c", "alice", "dave", 2, "alice", 20).await;
    for id in ["hidden"] {
        f.completed(id, "alice", "bob", 0, "alice", 30).await;
    }
    f.raw
        .execute_unprepared("UPDATE games SET visibility='private' WHERE id='hidden'")
        .await
        .unwrap();
    f.games
        .seed(
            WriteId::new("ongoing").unwrap(),
            Participants::new(
                UserId::new("ongoing-black").unwrap(),
                UserId::new("ongoing-white").unwrap(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let reader = f.reader().await;
    let first = reader.search(&search(filters(), 1, None)).await.unwrap();
    assert_eq!(ids(&first), ["c"]);
    let second = reader
        .search(&search(filters(), 1, first.next_cursor.as_deref()))
        .await
        .unwrap();
    assert_eq!(ids(&second), ["b"]);
    let third = reader
        .search(&search(filters(), 1, second.next_cursor.as_deref()))
        .await
        .unwrap();
    assert_eq!(ids(&third), ["a"]);
    assert!(third.next_cursor.is_none());
    for (change, expected) in [
        (
            Filters {
                participant: Some(UserId::new("carol").unwrap()),
                ..filters()
            },
            vec!["b"],
        ),
        (
            Filters {
                participant: Some(UserId::new("alice").unwrap()),
                ..filters()
            },
            vec!["c", "b", "a"],
        ),
        (
            Filters {
                winner: Some(Player::Black),
                ..filters()
            },
            vec!["b"],
        ),
        (
            Filters {
                winner: Some(Player::White),
                ..filters()
            },
            vec!["c", "a"],
        ),
        (
            Filters {
                reason: Some(FinishReason::Checkmate),
                ..filters()
            },
            vec![],
        ),
        (
            Filters {
                reason: Some(FinishReason::Resignation),
                ..filters()
            },
            vec!["c", "b", "a"],
        ),
        (
            Filters {
                rule: Some(Rule::Standard),
                ..filters()
            },
            vec!["c", "b", "a"],
        ),
        (
            Filters {
                from: 20,
                to: 21,
                ..filters()
            },
            vec!["c", "b"],
        ),
        (
            Filters {
                from: 0,
                to: 20,
                ..filters()
            },
            vec!["a"],
        ),
        (
            Filters {
                min_ply: Some(1),
                max_ply: Some(1),
                ..filters()
            },
            vec!["b"],
        ),
        (
            Filters {
                max_ply: Some(0),
                ..filters()
            },
            vec!["a"],
        ),
        (
            Filters {
                participant: Some(UserId::new("alice").unwrap()),
                rule: Some(Rule::Standard),
                winner: Some(Player::White),
                reason: Some(FinishReason::Resignation),
                from: 20,
                to: 21,
                min_ply: Some(2),
                max_ply: Some(2),
            },
            vec!["c"],
        ),
    ] {
        assert_eq!(
            ids(&reader.search(&search(change, 100, None)).await.unwrap()),
            expected
        );
    }
    for id in ["hidden", "ongoing", "missing"] {
        assert_eq!(
            reader.detail(&GameId::new(id).unwrap()).await,
            Err(Error::NotFound)
        );
    }
    let detail = reader.detail(&GameId::new("c").unwrap()).await.unwrap();
    assert_eq!(detail.metadata.final_move_ply().get(), 2);
    assert_eq!(detail.metadata.final_seq().get(), 3);
    assert_eq!(
        detail
            .history
            .iter()
            .map(|o| o.seq.get())
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(detail.history.last().unwrap().action, PublicAction::Resign);
    let mut changed = filters();
    changed.winner = Some(Player::White);
    assert!(Search::new(changed, 1, first.next_cursor.as_deref()).is_err());
    f.completed("new", "alice", "bob", 0, "alice", 40).await;
    assert_eq!(
        ids(&reader
            .search(&search(filters(), 1, first.next_cursor.as_deref()))
            .await
            .unwrap()),
        ["b"]
    );
    f.raw
        .execute_unprepared("UPDATE games SET visibility='private' WHERE id='b'")
        .await
        .unwrap();
    assert_eq!(
        ids(&reader
            .search(&search(filters(), 1, first.next_cursor.as_deref()))
            .await
            .unwrap()),
        ["a"]
    );
}
}
