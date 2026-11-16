use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の PostgreSQL で、識別子の順序とカーソルの往復を検証する。",
        reason: "符号化の境界事例を含む保存済み識別子でも、同値時のバイナリー順序とカーソルの往復を保持する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn identifier_encoding_preserves_binary_ties_and_cursor_round_trips() {
    let f = Fixture::new().await;
    let mut names = vec![
        "game\0",
        "game\u{1}",
        "game\u{1}\u{1}",
        "gameA",
        "game\\0",
        "game棋",
    ];
    for name in &names {
        f.completed(name, "black\0\u{1}", "white", 0, "white", 10)
            .await;
    }
    names.sort_by(|a, b| b.as_bytes().cmp(a.as_bytes()));
    let reader = f.reader().await;
    let mut cursor = None;
    let mut actual = Vec::new();
    loop {
        let mut query = filters();
        query.participant = Some(UserId::new("black\0\u{1}").unwrap());
        let page = reader
            .search(&search(query, 1, cursor.as_deref()))
            .await
            .unwrap();
        for item in page.items {
            let detail = reader.detail(item.id()).await.unwrap();
            assert_eq!(detail.metadata.id(), item.id());
            actual.push(item.id().as_str().to_string());
        }
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(actual, names);
}
}
