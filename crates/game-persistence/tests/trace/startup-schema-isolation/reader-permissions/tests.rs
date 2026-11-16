use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の PostgreSQL で、公開読み取り用の権限と、内部テーブルへのアクセスおよび更新の拒否を検証する。",
        reason: "実際の読み取りプロセスは四つの公開テーブルを SELECT でき、内部のレーティング・組合せテーブルへのアクセスを拒否され、rating_states を更新できない。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn actual_reader_role_can_read_only_the_public_rating_table() {
    let f = Fixture::new().await;
    f.create("permissions", Pool::TenMinute30).await;
    let reader_url = f.database.reader_url().await;
    let mut options = sea_orm::ConnectOptions::new(reader_url);
    options.max_connections(1);
    let connection = Database::connect(options).await.unwrap();
    connection
        .execute_unprepared("SET default_transaction_read_only=off")
        .await
        .unwrap();
    for table in ["schema_version", "games", "journal", "rating_states"] {
        connection
            .query_all_raw(Statement::from_string(
                DbBackend::Postgres,
                format!("SELECT * FROM {table}"),
            ))
            .await
            .unwrap();
    }
    for table in [
        "rated_games",
        "rating_slots",
        "rating_jobs",
        "rating_applications",
        "matchmaking_tickets",
    ] {
        let error = connection
            .query_all_raw(Statement::from_string(
                DbBackend::Postgres,
                format!("SELECT * FROM {table}"),
            ))
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("permission denied"),
            "{table}: {error}"
        );
    }
    assert!(
        connection
            .execute_unprepared("UPDATE rating_states SET rating=1500,completed=1")
            .await
            .unwrap_err()
            .to_string()
            .contains("permission denied")
    );
}
}
