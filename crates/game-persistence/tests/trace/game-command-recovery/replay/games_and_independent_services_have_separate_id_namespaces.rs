crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport.",
        reason: "Same request identity in separate games/services remains independent.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn games_and_independent_services_have_separate_id_namespaces() {
    let (_database, service) = setup().await;
    let (_other_database, other) = setup().await;
    let (a, b) = (id("a"), id("b"));
    let initial_a = service.seed(a.clone(), participants()).await.unwrap();
    let initial_b = service
        .seed(
            b.clone(),
            Participants::new(
                UserId::new("other-black").unwrap(),
                UserId::new("other-white").unwrap(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    other.seed(a.clone(), participants()).await.unwrap();
    service
        .make_move(&a, movement("shared", 0, Player::Black, "7g7f"))
        .await
        .unwrap();
    assert_eq!(service.snapshot(&b).await.unwrap(), initial_b);
    assert_eq!(other.snapshot(&a).await.unwrap(), initial_a);
    service
        .resign(
            &b,
            Resign {
                request_id: RequestId::new("shared").unwrap(),
                expected_seq: 0,
                user_id: UserId::new("other-white").unwrap(),
            },
        )
        .await
        .unwrap();
    assert!(service.snapshot(&a).await.unwrap().result.is_none());
    assert_eq!(service.snapshot(&id("missing")).await, Err(Error::NotFound));
    assert_eq!(
        service
            .make_move(&id("missing"), movement("m", 0, Player::Black, "7g7f"))
            .await,
        Err(Error::NotFound)
    );
    assert_eq!(
        service
            .resign(&id("missing"), resignation("r", 0, Player::White))
            .await,
        Err(Error::NotFound)
    );
}
}
