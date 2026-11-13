use crate::{SeaOrmStore, corrupt, decode_id, encode_id, schema::a, storage};
use async_trait::async_trait;
use game_application::rating::{
    ApplyOutcome, ClaimedRatingJob, JobFailure, RatedGame, RatingInput, RatingJobRepository,
    RatingWorkerError as WorkerError, RecoveryOutcome, TickOutcome, WorkerId, WorkerPolicy,
};
use game_application::{ApplicationError as Error, GameId, Participants, UserId};
use rating_domain::{Pool, RatingChange, RatingState};
use sea_orm::sea_query::{
    Expr, ExprTrait, JoinType, LockBehavior, LockType, OnConflict, Order, Query,
};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, QueryResult, TransactionTrait,
};

fn worker_error(error: Error) -> WorkerError {
    if error == Error::Corrupt {
        WorkerError::Corrupt
    } else {
        WorkerError::Unavailable
    }
}
fn unavailable(_: impl std::fmt::Debug) -> WorkerError {
    WorkerError::Unavailable
}
fn invalid(_: impl std::fmt::Debug) -> WorkerError {
    WorkerError::Corrupt
}
fn database_time() -> Expr {
    Expr::cust("floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint")
}
async fn now(tx: &DatabaseTransaction) -> Result<i64, WorkerError> {
    tx.query_one(Query::select().expr_as(database_time(), a("now")))
        .await
        .map_err(unavailable)?
        .ok_or(WorkerError::Corrupt)?
        .try_get("", "now")
        .map_err(invalid)
}
fn state(row: &QueryResult, rating: &str, count: &str) -> Result<RatingState, Error> {
    RatingState::from_parts(
        row.try_get("", rating).map_err(corrupt)?,
        row.try_get("", count).map_err(corrupt)?,
    )
    .map_err(corrupt)
}
pub(crate) async fn identity(
    db: &impl ConnectionTrait,
    id: &GameId,
) -> Result<Option<RatedGame>, Error> {
    let row = db
        .query_one(
            Query::select()
                .columns([
                    a("black"),
                    a("white"),
                    a("pool"),
                    a("black_start_rating"),
                    a("black_start_count"),
                    a("white_start_rating"),
                    a("white_start_count"),
                ])
                .from(a("rated_games"))
                .join(
                    JoinType::InnerJoin,
                    a("games"),
                    Expr::col((a("rated_games"), a("game_id")))
                        .eq(Expr::col((a("games"), a("id")))),
                )
                .and_where(Expr::col(a("game_id")).eq(encode_id(id.as_str()))),
        )
        .await
        .map_err(storage)?;
    row.map(|row| {
        Ok(RatedGame {
            game_id: id.clone(),
            participants: Participants::new(
                UserId::new(decode_id(
                    &row.try_get::<String>("", "black").map_err(corrupt)?,
                )?)
                .map_err(corrupt)?,
                UserId::new(decode_id(
                    &row.try_get::<String>("", "white").map_err(corrupt)?,
                )?)
                .map_err(corrupt)?,
            )
            .map_err(corrupt)?,
            pool: Pool::parse(&row.try_get::<String>("", "pool").map_err(corrupt)?)
                .map_err(corrupt)?,
            black_start: state(&row, "black_start_rating", "black_start_count")?,
            white_start: state(&row, "white_start_rating", "white_start_count")?,
        })
    })
    .transpose()
}
fn sorted_users(players: &Participants) -> [&UserId; 2] {
    let mut users = [players.black(), players.white()];
    users.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    users
}
async fn locked_states(
    tx: &DatabaseTransaction,
    players: &Participants,
    pool: Pool,
) -> Result<[RatingState; 2], Error> {
    let mut black = None;
    let mut white = None;
    for user in sorted_users(players) {
        let row = tx
            .query_one(
                Query::select()
                    .columns([a("rating"), a("completed")])
                    .from(a("rating_states"))
                    .and_where(Expr::col(a("user_id")).eq(encode_id(user.as_str())))
                    .and_where(Expr::col(a("pool")).eq(pool.as_str()))
                    .lock_exclusive(),
            )
            .await
            .map_err(storage)?
            .ok_or(Error::Corrupt)?;
        let value = state(&row, "rating", "completed")?;
        if user == players.black() {
            black = Some(value)
        } else {
            white = Some(value)
        };
    }
    Ok([black.ok_or(Error::Corrupt)?, white.ok_or(Error::Corrupt)?])
}
pub(crate) async fn admit(
    db: &DatabaseConnection,
    id: GameId,
    players: Participants,
    pool: Pool,
) -> Result<(), Error> {
    let tx = db.begin().await.map_err(storage)?;
    for user in sorted_users(&players) {
        tx.execute(
            Query::insert()
                .into_table(a("rating_states"))
                .columns([a("user_id"), a("pool"), a("rating"), a("completed")])
                .values_panic([
                    encode_id(user.as_str()).into(),
                    pool.as_str().into(),
                    Option::<i64>::None.into(),
                    0_i64.into(),
                ])
                .on_conflict(
                    OnConflict::columns([a("user_id"), a("pool")])
                        .do_nothing()
                        .to_owned(),
                ),
        )
        .await
        .map_err(storage)?;
    }
    let starts = locked_states(&tx, &players, pool).await?;
    if tx
        .query_one(
            Query::select()
                .column(a("id"))
                .from(a("games"))
                .and_where(Expr::col(a("id")).eq(encode_id(id.as_str()))),
        )
        .await
        .map_err(storage)?
        .is_some()
    {
        return Err(Error::AlreadyExists);
    }
    for user in sorted_users(&players) {
        if tx
            .query_one(
                Query::select()
                    .column(a("game_id"))
                    .from(a("rating_slots"))
                    .and_where(Expr::col(a("user_id")).eq(encode_id(user.as_str())))
                    .and_where(Expr::col(a("pool")).eq(pool.as_str())),
            )
            .await
            .map_err(storage)?
            .is_some()
        {
            return Err(Error::RatingPoolBusy);
        }
    }
    crate::insert_game(&tx, &id, &players).await?;
    tx.execute(
        Query::insert()
            .into_table(a("rated_games"))
            .columns([
                a("game_id"),
                a("pool"),
                a("black_start_rating"),
                a("black_start_count"),
                a("white_start_rating"),
                a("white_start_count"),
            ])
            .values_panic([
                encode_id(id.as_str()).into(),
                pool.as_str().into(),
                starts[0].rating().into(),
                (starts[0].completed() as i64).into(),
                starts[1].rating().into(),
                (starts[1].completed() as i64).into(),
            ]),
    )
    .await
    .map_err(storage)?;
    for user in sorted_users(&players) {
        tx.execute(
            Query::insert()
                .into_table(a("rating_slots"))
                .columns([a("user_id"), a("pool"), a("game_id")])
                .values_panic([
                    encode_id(user.as_str()).into(),
                    pool.as_str().into(),
                    encode_id(id.as_str()).into(),
                ]),
        )
        .await
        .map_err(storage)?;
    }
    tx.commit().await.map_err(storage)
}
pub(crate) async fn enqueue(tx: &DatabaseTransaction, id: &GameId) -> Result<(), Error> {
    if identity(tx, id).await?.is_none() {
        return Ok(());
    }
    let time = now(tx).await.map_err(|_| Error::Unavailable)?;
    tx.execute(
        Query::insert()
            .into_table(a("rating_jobs"))
            .columns([
                a("game_id"),
                a("state"),
                a("generation"),
                a("available_at"),
                a("attempts"),
            ])
            .values_panic([
                encode_id(id.as_str()).into(),
                "pending".into(),
                0_i64.into(),
                time.into(),
                0_i64.into(),
            ]),
    )
    .await
    .map_err(storage)?;
    Ok(())
}
async fn input(tx: &DatabaseTransaction, id: &GameId) -> Result<RatingInput, WorkerError> {
    let game = identity(tx, id)
        .await
        .map_err(worker_error)?
        .ok_or(WorkerError::Corrupt)?;
    let row = tx
        .query_one(
            Query::select()
                .columns([a("version"), a("winner"), a("reason")])
                .from(a("games"))
                .and_where(Expr::col(a("id")).eq(encode_id(id.as_str()))),
        )
        .await
        .map_err(unavailable)?
        .ok_or(WorkerError::Corrupt)?;
    let result = crate::result(&row, "")
        .map_err(worker_error)?
        .ok_or(WorkerError::Corrupt)?;
    let seq: i64 = row.try_get("", "version").map_err(invalid)?;
    if seq <= 0 {
        return Err(WorkerError::Corrupt);
    }
    Ok(RatingInput {
        game,
        final_seq: seq as u64,
        winner: result.winner,
    })
}
async fn job(tx: &DatabaseTransaction, id: &GameId) -> Result<Option<QueryResult>, WorkerError> {
    tx.query_one(
        Query::select()
            .columns([
                a("state"),
                a("owner"),
                a("generation"),
                a("lease_until"),
                a("attempts"),
            ])
            .from(a("rating_jobs"))
            .and_where(Expr::col(a("game_id")).eq(encode_id(id.as_str())))
            .lock_exclusive(),
    )
    .await
    .map_err(unavailable)
}
fn fenced(row: &QueryResult, claim: &ClaimedRatingJob, time: i64) -> Result<bool, WorkerError> {
    Ok(
        row.try_get::<String>("", "state").map_err(invalid)? == "claimed"
            && row
                .try_get::<Option<String>>("", "owner")
                .map_err(invalid)?
                .as_deref()
                == Some(claim.owner().as_str())
            && row.try_get::<i64>("", "generation").map_err(invalid)? == claim.generation()
            && row
                .try_get::<Option<i64>>("", "lease_until")
                .map_err(invalid)?
                .is_some_and(|end| end > time),
    )
}
/// Durable rating jobs sharing the game database and schema contract.
pub struct SeaOrmRatingJobs {
    store: SeaOrmStore,
}
impl SeaOrmRatingJobs {
    /// Initializes or validates schema v6 before opening worker operations.
    pub async fn connect(url: &str) -> Result<Self, Error> {
        Ok(Self {
            store: SeaOrmStore::connect(url).await?,
        })
    }
    /// Closes this worker's connection pool.
    pub async fn close(&self) -> Result<(), Error> {
        self.store.close().await
    }
}
#[async_trait]
impl RatingJobRepository for SeaOrmRatingJobs {
    async fn claim(
        &self,
        owner: &WorkerId,
        policy: &WorkerPolicy,
    ) -> Result<Option<ClaimedRatingJob>, WorkerError> {
        for _ in 0..32 {
            let tx = self.store.db.begin().await.map_err(unavailable)?;
            let time = now(&tx).await?;
            let row = tx
                .query_one(
                    Query::select()
                        .columns([a("game_id"), a("generation"), a("attempts")])
                        .from(a("rating_jobs"))
                        .and_where(
                            Expr::col(a("state"))
                                .eq("pending")
                                .and(Expr::col(a("available_at")).lte(time))
                                .or(Expr::col(a("state"))
                                    .eq("claimed")
                                    .and(Expr::col(a("lease_until")).lte(time))),
                        )
                        .order_by(a("available_at"), Order::Asc)
                        .order_by(a("game_id"), Order::Asc)
                        .limit(1)
                        .lock_with_behavior(LockType::Update, LockBehavior::SkipLocked),
                )
                .await
                .map_err(unavailable)?;
            let Some(row) = row else {
                tx.commit().await.map_err(unavailable)?;
                return Ok(None);
            };
            let id = GameId::new(
                decode_id(&row.try_get::<String>("", "game_id").map_err(invalid)?)
                    .map_err(worker_error)?,
            )
            .map_err(invalid)?;
            let generation = row
                .try_get::<i64>("", "generation")
                .map_err(invalid)?
                .checked_add(1)
                .ok_or(WorkerError::Corrupt)?;
            let attempts: i64 = row.try_get("", "attempts").map_err(invalid)?;
            if attempts >= i64::from(policy.retry_limit()) {
                tx.execute(
                    Query::update()
                        .table(a("rating_jobs"))
                        .value(a("state"), "quarantined")
                        .value(a("owner"), Option::<String>::None)
                        .value(a("lease_until"), Option::<i64>::None)
                        .value(a("generation"), generation)
                        .value(
                            a("last_error"),
                            "retry budget exhausted, including expired claims",
                        )
                        .and_where(Expr::col(a("game_id")).eq(encode_id(id.as_str()))),
                )
                .await
                .map_err(unavailable)?;
                tx.commit().await.map_err(unavailable)?;
                continue;
            }
            let lease = now(&tx)
                .await?
                .checked_add(policy.lease_millis())
                .ok_or(WorkerError::InvalidConfiguration)?;
            tx.execute(
                Query::update()
                    .table(a("rating_jobs"))
                    .value(a("state"), "claimed")
                    .value(a("owner"), owner.as_str())
                    .value(a("generation"), generation)
                    .value(a("attempts"), attempts + 1)
                    .value(a("lease_until"), lease)
                    .and_where(Expr::col(a("game_id")).eq(encode_id(id.as_str()))),
            )
            .await
            .map_err(unavailable)?;
            let input = match input(&tx, &id).await {
                Ok(i) => Ok(i),
                Err(WorkerError::Corrupt) => Err(JobFailure::Corrupt),
                Err(e) => return Err(e),
            };
            tx.commit().await.map_err(unavailable)?;
            return Ok(Some(ClaimedRatingJob::new(
                id,
                owner.clone(),
                generation,
                input,
            )?));
        }
        Ok(None)
    }
    async fn apply(
        &self,
        claim: &ClaimedRatingJob,
        change: &RatingChange,
    ) -> Result<ApplyOutcome, WorkerError> {
        let tx = self.store.db.begin().await.map_err(unavailable)?;
        let Some(row) = job(&tx, claim.game_id()).await? else {
            return Ok(ApplyOutcome::LostClaim);
        };
        let status: String = row.try_get("", "state").map_err(invalid)?;
        if status != "completed" && !fenced(&row, claim, now(&tx).await?)? {
            return Ok(ApplyOutcome::LostClaim);
        }
        let input = input(&tx, claim.game_id()).await?;
        if claim.input().map_err(|_| WorkerError::Corrupt)? != &input
            || rating_domain::calculate(
                input.game.black_start,
                input.game.white_start,
                input.winner,
            )
            .map_err(invalid)?
                != *change
        {
            return Err(WorkerError::Corrupt);
        }
        let ledger = tx
            .query_one(
                Query::select()
                    .columns([
                        a("pool"),
                        a("final_seq"),
                        a("delta_black"),
                        a("black_before_rating"),
                        a("black_before_count"),
                        a("white_before_rating"),
                        a("white_before_count"),
                        a("black_after_rating"),
                        a("black_after_count"),
                        a("white_after_rating"),
                        a("white_after_count"),
                    ])
                    .from(a("rating_applications"))
                    .and_where(Expr::col(a("game_id")).eq(encode_id(claim.game_id().as_str()))),
            )
            .await
            .map_err(unavailable)?;
        if status == "completed" {
            let ledger = ledger.ok_or(WorkerError::Corrupt)?;
            if ledger.try_get::<String>("", "pool").map_err(invalid)? != input.game.pool.as_str()
                || ledger.try_get::<i64>("", "final_seq").map_err(invalid)?
                    != input.final_seq as i64
                || ledger.try_get::<i64>("", "delta_black").map_err(invalid)? != change.delta_black
                || state(&ledger, "black_before_rating", "black_before_count")
                    .map_err(worker_error)?
                    != input.game.black_start
                || state(&ledger, "white_before_rating", "white_before_count")
                    .map_err(worker_error)?
                    != input.game.white_start
                || state(&ledger, "black_after_rating", "black_after_count")
                    .map_err(worker_error)?
                    != change.black
                || state(&ledger, "white_after_rating", "white_after_count")
                    .map_err(worker_error)?
                    != change.white
            {
                return Err(WorkerError::Corrupt);
            }
            tx.commit().await.map_err(unavailable)?;
            return Ok(ApplyOutcome::AlreadyApplied);
        }
        if ledger.is_some() {
            return Err(WorkerError::Corrupt);
        }
        let starts = locked_states(&tx, &input.game.participants, input.game.pool)
            .await
            .map_err(worker_error)?;
        if starts != [input.game.black_start, input.game.white_start] {
            return Err(WorkerError::Corrupt);
        }
        for (user, new) in [
            (input.game.participants.black(), change.black),
            (input.game.participants.white(), change.white),
        ] {
            let slot = tx
                .query_one(
                    Query::select()
                        .column(a("game_id"))
                        .from(a("rating_slots"))
                        .and_where(Expr::col(a("user_id")).eq(encode_id(user.as_str())))
                        .and_where(Expr::col(a("pool")).eq(input.game.pool.as_str()))
                        .lock_exclusive(),
                )
                .await
                .map_err(unavailable)?
                .ok_or(WorkerError::Corrupt)?;
            if slot.try_get::<String>("", "game_id").map_err(invalid)?
                != encode_id(claim.game_id().as_str())
            {
                return Err(WorkerError::Corrupt);
            }
            let updated = tx
                .execute(
                    Query::update()
                        .table(a("rating_states"))
                        .value(a("rating"), new.rating())
                        .value(a("completed"), new.completed() as i64)
                        .and_where(Expr::col(a("user_id")).eq(encode_id(user.as_str())))
                        .and_where(Expr::col(a("pool")).eq(input.game.pool.as_str())),
                )
                .await
                .map_err(unavailable)?;
            if updated.rows_affected() != 1 {
                return Err(WorkerError::Corrupt);
            }
            let deleted = tx
                .execute(
                    Query::delete()
                        .from_table(a("rating_slots"))
                        .and_where(Expr::col(a("user_id")).eq(encode_id(user.as_str())))
                        .and_where(Expr::col(a("pool")).eq(input.game.pool.as_str()))
                        .and_where(Expr::col(a("game_id")).eq(encode_id(claim.game_id().as_str()))),
                )
                .await
                .map_err(unavailable)?;
            if deleted.rows_affected() != 1 {
                return Err(WorkerError::Corrupt);
            }
        }
        tx.execute(
            Query::insert()
                .into_table(a("rating_applications"))
                .columns([
                    a("game_id"),
                    a("pool"),
                    a("final_seq"),
                    a("delta_black"),
                    a("black_before_rating"),
                    a("black_before_count"),
                    a("white_before_rating"),
                    a("white_before_count"),
                    a("black_after_rating"),
                    a("black_after_count"),
                    a("white_after_rating"),
                    a("white_after_count"),
                ])
                .values_panic([
                    encode_id(claim.game_id().as_str()).into(),
                    input.game.pool.as_str().into(),
                    (input.final_seq as i64).into(),
                    change.delta_black.into(),
                    input.game.black_start.rating().into(),
                    (input.game.black_start.completed() as i64).into(),
                    input.game.white_start.rating().into(),
                    (input.game.white_start.completed() as i64).into(),
                    change.black.rating().into(),
                    (change.black.completed() as i64).into(),
                    change.white.rating().into(),
                    (change.white.completed() as i64).into(),
                ]),
        )
        .await
        .map_err(unavailable)?;
        let updated = tx
            .execute(
                Query::update()
                    .table(a("rating_jobs"))
                    .value(a("state"), "completed")
                    .value(a("owner"), Option::<String>::None)
                    .value(a("lease_until"), Option::<i64>::None)
                    .value(a("last_error"), Option::<String>::None)
                    .and_where(Expr::col(a("game_id")).eq(encode_id(claim.game_id().as_str())))
                    .and_where(Expr::col(a("state")).eq("claimed"))
                    .and_where(Expr::col(a("owner")).eq(claim.owner().as_str()))
                    .and_where(Expr::col(a("generation")).eq(claim.generation()))
                    .and_where(Expr::col(a("lease_until")).gt(database_time())),
            )
            .await
            .map_err(unavailable)?;
        if updated.rows_affected() != 1 {
            tx.rollback().await.map_err(unavailable)?;
            return Ok(ApplyOutcome::LostClaim);
        }
        tx.commit().await.map_err(unavailable)?;
        Ok(ApplyOutcome::Applied)
    }
    async fn fail(
        &self,
        claim: &ClaimedRatingJob,
        failure: JobFailure,
        policy: &WorkerPolicy,
    ) -> Result<TickOutcome, WorkerError> {
        let tx = self.store.db.begin().await.map_err(unavailable)?;
        let Some(row) = job(&tx, claim.game_id()).await? else {
            return Ok(TickOutcome::LostClaim);
        };
        let time = now(&tx).await?;
        if !fenced(&row, claim, time)? {
            return Ok(TickOutcome::LostClaim);
        }
        let quarantine = failure == JobFailure::Corrupt
            || row.try_get::<i64>("", "attempts").map_err(invalid)?
                >= i64::from(policy.retry_limit());
        let available = time
            .checked_add(policy.backoff_millis())
            .ok_or(WorkerError::InvalidConfiguration)?;
        let updated = tx
            .execute(
                Query::update()
                    .table(a("rating_jobs"))
                    .value(
                        a("state"),
                        if quarantine { "quarantined" } else { "pending" },
                    )
                    .value(a("owner"), Option::<String>::None)
                    .value(a("lease_until"), Option::<i64>::None)
                    .value(a("available_at"), available)
                    .value(
                        a("last_error"),
                        if failure == JobFailure::Corrupt {
                            "corrupt input, state, ledger or reservation"
                        } else {
                            "transient storage failure"
                        },
                    )
                    .and_where(Expr::col(a("game_id")).eq(encode_id(claim.game_id().as_str())))
                    .and_where(Expr::col(a("generation")).eq(claim.generation()))
                    .and_where(Expr::col(a("owner")).eq(claim.owner().as_str()))
                    .and_where(Expr::col(a("lease_until")).gt(database_time())),
            )
            .await
            .map_err(unavailable)?;
        if updated.rows_affected() != 1 {
            return Ok(TickOutcome::LostClaim);
        }
        tx.commit().await.map_err(unavailable)?;
        Ok(if quarantine {
            TickOutcome::Quarantined
        } else {
            TickOutcome::Deferred
        })
    }
    async fn retry_quarantined(&self, id: &GameId) -> Result<RecoveryOutcome, WorkerError> {
        let tx = self.store.db.begin().await.map_err(unavailable)?;
        let Some(row) = job(&tx, id).await? else {
            return Ok(RecoveryOutcome::NotFound);
        };
        if row.try_get::<String>("", "state").map_err(invalid)? != "quarantined" {
            return Ok(RecoveryOutcome::NotQuarantined);
        }
        let generation = row
            .try_get::<i64>("", "generation")
            .map_err(invalid)?
            .checked_add(1)
            .ok_or(WorkerError::Corrupt)?;
        let time = now(&tx).await?;
        tx.execute(
            Query::update()
                .table(a("rating_jobs"))
                .value(a("state"), "pending")
                .value(a("attempts"), 0_i64)
                .value(a("generation"), generation)
                .value(a("available_at"), time)
                .value(a("last_error"), Option::<String>::None)
                .and_where(Expr::col(a("game_id")).eq(encode_id(id.as_str())))
                .and_where(Expr::col(a("state")).eq("quarantined")),
        )
        .await
        .map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(RecoveryOutcome::Retried)
    }
}
