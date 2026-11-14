use crate::{SeaOrmStore, corrupt, decode_id, encode_id, rating, schema::a, storage};
use async_trait::async_trait;
use game_application::matchmaking::{
    HeartbeatSequence, JoinOutcome, MatchmakingError as MError, MatchmakingRepository,
    Participation, RefusalReason, Ticket, TicketId, TicketState, Waiting, select_pair,
};
use game_application::{ApplicationError as Error, GameId, Participants, UserId};
use rating_domain::{Pool, RatingState};
use sea_orm::sea_query::{Expr, ExprTrait, OnConflict, Order, Query};
use sea_orm::{ConnectionTrait, DatabaseTransaction, QueryResult, TransactionTrait};
use shogi_domain::Player;

pub(crate) async fn admission_lock(tx: &DatabaseTransaction) -> Result<(), Error> {
    tx.execute_unprepared("SELECT pg_advisory_xact_lock(6001133723959083849)")
        .await
        .map_err(storage)?;
    Ok(())
}
async fn now(tx: &DatabaseTransaction) -> Result<i64, Error> {
    tx.query_one(Query::select().expr_as(
        Expr::cust("floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint"),
        a("now"),
    ))
    .await
    .map_err(storage)?
    .ok_or(Error::Corrupt)?
    .try_get("", "now")
    .map_err(corrupt)
}
fn owner(user: &UserId) -> Expr {
    Expr::col(a("user_id")).eq(encode_id(user.as_str()))
}
fn identity(user: &UserId, id: TicketId) -> Expr {
    owner(user).and(Expr::col(a("ticket_id")).eq(id.to_string()))
}
fn decode(row: QueryResult) -> Result<Ticket, Error> {
    let pool = row
        .try_get::<Option<String>>("", "pool")
        .map_err(corrupt)?
        .map(|s| Pool::parse(&s).map_err(corrupt))
        .transpose()?;
    let game = || -> Result<(GameId, Player), Error> {
        Ok((
            GameId::new(decode_id(
                &row.try_get::<String>("", "game_id").map_err(corrupt)?,
            )?)
            .map_err(corrupt)?,
            match row.try_get::<i32>("", "side").map_err(corrupt)? {
                1 => Player::Black,
                2 => Player::White,
                _ => return Err(Error::Corrupt),
            },
        ))
    };
    let state = match row
        .try_get::<String>("", "state")
        .map_err(corrupt)?
        .as_str()
    {
        "waiting" => TicketState::Waiting(Waiting {
            pool: pool.ok_or(Error::Corrupt)?,
            rating: RatingState::from_parts(
                row.try_get("", "rating").map_err(corrupt)?,
                row.try_get("", "completed").map_err(corrupt)?,
            )
            .map_err(corrupt)?,
            enqueued_at: row.try_get("", "enqueued_at").map_err(corrupt)?,
            expires_at: row.try_get("", "expires_at").map_err(corrupt)?,
            heartbeat_seq: row.try_get("", "heartbeat_seq").map_err(corrupt)?,
        }),
        "playing" => {
            let (game_id, side) = game()?;
            TicketState::Playing { game_id, side }
        }
        "finished" => {
            let (game_id, side) = game()?;
            TicketState::Finished { game_id, side }
        }
        "cancelled" => TicketState::Cancelled,
        "expired" => TicketState::Expired,
        "refused" => TicketState::Refused(
            match row
                .try_get::<String>("", "refusal_reason")
                .map_err(corrupt)?
                .as_str()
            {
                "already_waiting" => RefusalReason::AlreadyWaiting,
                "already_playing" => RefusalReason::AlreadyPlaying,
                "rating_pending" => RefusalReason::RatingPending,
                _ => return Err(Error::Corrupt),
            },
        ),
        _ => return Err(Error::Corrupt),
    };
    Ok(Ticket {
        user: UserId::new(decode_id(
            &row.try_get::<String>("", "user_id").map_err(corrupt)?,
        )?)
        .map_err(corrupt)?,
        id: TicketId::parse(&row.try_get::<String>("", "ticket_id").map_err(corrupt)?)
            .map_err(corrupt)?,
        pool,
        state,
    })
}
async fn read(
    tx: &DatabaseTransaction,
    user: &UserId,
    id: TicketId,
) -> Result<Option<Ticket>, Error> {
    tx.query_one(
        Query::select()
            .column(sea_orm::sea_query::Asterisk)
            .from(a("matchmaking_tickets"))
            .and_where(identity(user, id))
            .lock_exclusive(),
    )
    .await
    .map_err(storage)?
    .map(decode)
    .transpose()
}
async fn expire(tx: &DatabaseTransaction, user: Option<&UserId>) -> Result<(), Error> {
    let mut locks = Query::select();
    locks
        .column(a("ticket_id"))
        .from(a("matchmaking_tickets"))
        .and_where(Expr::col(a("state")).eq("waiting"))
        .order_by(a("user_id"), Order::Asc)
        .order_by(a("ticket_id"), Order::Asc)
        .lock_exclusive();
    if let Some(user) = user {
        locks.and_where(owner(user));
    }
    tx.query_all(&locks).await.map_err(storage)?;
    let mut query = Query::update();
    query
        .table(a("matchmaking_tickets"))
        .value(a("state"), "expired")
        .and_where(Expr::col(a("state")).eq("waiting"))
        .and_where(Expr::col(a("expires_at")).lte(now(tx).await?));
    if let Some(user) = user {
        query.and_where(owner(user));
    }
    tx.execute(&query).await.map_err(storage)?;
    Ok(())
}
async fn active(tx: &DatabaseTransaction, user: &UserId) -> Result<Option<Ticket>, Error> {
    tx.query_one(
        Query::select()
            .column(sea_orm::sea_query::Asterisk)
            .from(a("matchmaking_tickets"))
            .and_where(owner(user))
            .and_where(Expr::col(a("state")).is_in(["waiting", "playing"]))
            .lock_exclusive(),
    )
    .await
    .map_err(storage)?
    .map(decode)
    .transpose()
}
async fn locked_rating(
    tx: &DatabaseTransaction,
    user: &UserId,
    pool: Pool,
) -> Result<RatingState, Error> {
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
    let row = tx
        .query_one(
            Query::select()
                .columns([a("rating"), a("completed")])
                .from(a("rating_states"))
                .and_where(owner(user))
                .and_where(Expr::col(a("pool")).eq(pool.as_str()))
                .lock_exclusive(),
        )
        .await
        .map_err(storage)?
        .ok_or(Error::Corrupt)?;
    RatingState::from_parts(
        row.try_get("", "rating").map_err(corrupt)?,
        row.try_get("", "completed").map_err(corrupt)?,
    )
    .map_err(corrupt)
}
async fn reserved(tx: &DatabaseTransaction, user: &UserId, pool: Pool) -> Result<bool, Error> {
    Ok(tx
        .query_one(
            Query::select()
                .column(a("game_id"))
                .from(a("rating_slots"))
                .and_where(owner(user))
                .and_where(Expr::col(a("pool")).eq(pool.as_str())),
        )
        .await
        .map_err(storage)?
        .is_some())
}
async fn begin(store: &SeaOrmStore) -> Result<DatabaseTransaction, Error> {
    let tx = store.db.begin().await.map_err(storage)?;
    admission_lock(&tx).await?;
    Ok(tx)
}

#[async_trait]
impl MatchmakingRepository for SeaOrmStore {
    async fn join(&self, user: UserId, id: TicketId, pool: Pool) -> Result<JoinOutcome, MError> {
        let tx = begin(self).await?;
        expire(&tx, Some(&user)).await?;
        if let Some(ticket) = read(&tx, &user, id).await? {
            if ticket.pool.is_some_and(|old| old != pool) {
                return Err(MError::PoolConflict);
            }
            tx.commit().await.map_err(storage)?;
            return Ok(JoinOutcome {
                ticket,
                created: false,
            });
        }
        let reason = match active(&tx, &user).await?.map(|t| t.state) {
            Some(TicketState::Waiting(_)) => Some(RefusalReason::AlreadyWaiting),
            Some(TicketState::Playing { .. }) => Some(RefusalReason::AlreadyPlaying),
            Some(_) => return Err(MError::Corrupt),
            None => None,
        };
        let rating = locked_rating(&tx, &user, pool).await?;
        let reason = if reason.is_some() {
            reason
        } else if reserved(&tx, &user, pool).await? {
            Some(RefusalReason::RatingPending)
        } else {
            None
        };
        let mut insert = Query::insert();
        insert.into_table(a("matchmaking_tickets"));
        let mut columns = vec![
            a("user_id"),
            a("ticket_id"),
            a("pool"),
            a("state"),
            a("refusal_reason"),
        ];
        let mut values = vec![
            encode_id(user.as_str()).into(),
            id.to_string().into(),
            pool.as_str().into(),
            if reason.is_some() {
                "refused"
            } else {
                "waiting"
            }
            .into(),
            reason.map(|r| r.as_str().to_owned()).into(),
        ];
        if reason.is_none() {
            let time = now(&tx).await?;
            columns.extend([
                a("enqueued_at"),
                a("expires_at"),
                a("rating"),
                a("completed"),
                a("heartbeat_seq"),
            ]);
            values.extend([
                time.into(),
                (time + 30_000).into(),
                rating.rating().into(),
                (rating.completed() as i64).into(),
                0_i64.into(),
            ]);
        }
        tx.execute(insert.columns(columns).values_panic(values))
            .await
            .map_err(storage)?;
        let ticket = read(&tx, &user, id).await?.ok_or(MError::Corrupt)?;
        tx.commit().await.map_err(storage)?;
        Ok(JoinOutcome {
            ticket,
            created: reason.is_none(),
        })
    }
    async fn current(&self, user: &UserId) -> Result<Participation, MError> {
        let tx = begin(self).await?;
        expire(&tx, Some(user)).await?;
        let result = active(&tx, user)
            .await?
            .map_or(Participation::Idle, Participation::Active);
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
    async fn ticket(&self, user: &UserId, id: TicketId) -> Result<Ticket, MError> {
        let tx = begin(self).await?;
        expire(&tx, Some(user)).await?;
        let ticket = read(&tx, user, id).await?.ok_or(MError::NotFound)?;
        tx.commit().await.map_err(storage)?;
        Ok(ticket)
    }
    async fn heartbeat(
        &self,
        user: &UserId,
        id: TicketId,
        sequence: HeartbeatSequence,
    ) -> Result<Ticket, MError> {
        let tx = begin(self).await?;
        let ticket = read(&tx, user, id).await?.ok_or(MError::NotFound)?;
        let time = now(&tx).await?;
        if let TicketState::Waiting(waiting) = ticket.state {
            let mut update = Query::update();
            update
                .table(a("matchmaking_tickets"))
                .and_where(identity(user, id));
            if waiting.expires_at <= time {
                update.value(a("state"), "expired");
            } else if sequence.get() > waiting.heartbeat_seq {
                update
                    .value(a("expires_at"), time + 30_000)
                    .value(a("heartbeat_seq"), sequence.get());
            } else {
                tx.commit().await.map_err(storage)?;
                return Ok(Ticket {
                    state: TicketState::Waiting(waiting),
                    ..ticket
                });
            }
            tx.execute(&update).await.map_err(storage)?;
        }
        let ticket = read(&tx, user, id).await?.ok_or(MError::Corrupt)?;
        tx.commit().await.map_err(storage)?;
        Ok(ticket)
    }
    async fn cancel(&self, user: &UserId, id: TicketId) -> Result<Ticket, MError> {
        let tx = begin(self).await?;
        let ticket = read(&tx, user, id).await?;
        if let Some(ticket) = ticket {
            if let TicketState::Waiting(waiting) = ticket.state {
                let state = if waiting.expires_at <= now(&tx).await? {
                    "expired"
                } else {
                    "cancelled"
                };
                tx.execute(
                    Query::update()
                        .table(a("matchmaking_tickets"))
                        .value(a("state"), state)
                        .and_where(identity(user, id)),
                )
                .await
                .map_err(storage)?;
            }
        } else {
            tx.execute(
                Query::insert()
                    .into_table(a("matchmaking_tickets"))
                    .columns([a("user_id"), a("ticket_id"), a("state")])
                    .values_panic([
                        encode_id(user.as_str()).into(),
                        id.to_string().into(),
                        "cancelled".into(),
                    ]),
            )
            .await
            .map_err(storage)?;
        }
        let ticket = read(&tx, user, id).await?.ok_or(MError::Corrupt)?;
        tx.commit().await.map_err(storage)?;
        Ok(ticket)
    }
    async fn tick(&self, pool: Pool) -> Result<Option<GameId>, MError> {
        let tx = begin(self).await?;
        expire(&tx, None).await?;
        let tickets = tx
            .query_all(
                Query::select()
                    .column(sea_orm::sea_query::Asterisk)
                    .from(a("matchmaking_tickets"))
                    .and_where(Expr::col(a("state")).eq("waiting"))
                    .and_where(Expr::col(a("pool")).eq(pool.as_str()))
                    .order_by(a("enqueued_at"), Order::Asc)
                    .order_by(a("ticket_id"), Order::Asc),
            )
            .await
            .map_err(storage)?
            .into_iter()
            .map(decode)
            .collect::<Result<Vec<_>, _>>()?;
        let Some((first, second)) = select_pair(&tickets, now(&tx).await?) else {
            tx.commit().await.map_err(storage)?;
            return Ok(None);
        };
        let mut selected = [first, second];
        selected.sort_by_key(|t| (t.user.as_str(), t.id));
        let mut locked = Vec::new();
        for ticket in selected {
            locked.push(
                read(&tx, &ticket.user, ticket.id)
                    .await?
                    .ok_or(MError::Corrupt)?,
            );
        }
        let flip = uuid::Uuid::new_v4().as_bytes()[0] & 1 == 0;
        let players = if flip {
            Participants::new(first.user.clone(), second.user.clone())
        } else {
            Participants::new(second.user.clone(), first.user.clone())
        }
        .map_err(|_| MError::Corrupt)?;
        let starts = rating::locked_states(&tx, &players, pool).await?;
        for ticket in &locked {
            let TicketState::Waiting(waiting) = &ticket.state else {
                return Err(MError::Corrupt);
            };
            let index = usize::from(&ticket.user != players.black());
            if starts[index] != waiting.rating || reserved(&tx, &ticket.user, pool).await? {
                return Err(MError::Corrupt);
            }
        }
        let time = now(&tx).await?;
        if select_pair(&locked, time).is_none() {
            expire(&tx, None).await?;
            tx.commit().await.map_err(storage)?;
            return Ok(None);
        }
        let id = GameId::new(uuid::Uuid::new_v4().to_string()).map_err(|_| MError::Corrupt)?;
        rating::insert_rated(&tx, &id, &players, pool, starts).await?;
        for ticket in &locked {
            tx.execute(
                Query::update()
                    .table(a("matchmaking_tickets"))
                    .value(a("state"), "playing")
                    .value(a("game_id"), encode_id(id.as_str()))
                    .value(
                        a("side"),
                        if &ticket.user == players.black() {
                            1_i32
                        } else {
                            2_i32
                        },
                    )
                    .and_where(identity(&ticket.user, ticket.id)),
            )
            .await
            .map_err(storage)?;
        }
        tx.commit().await.map_err(storage)?;
        Ok(Some(id))
    }
}

pub(crate) async fn finish(
    tx: &DatabaseTransaction,
    id: &GameId,
    black: &str,
    white: &str,
) -> Result<(), Error> {
    let rows = tx
        .query_all(
            Query::select()
                .column(sea_orm::sea_query::Asterisk)
                .from(a("matchmaking_tickets"))
                .and_where(Expr::col(a("game_id")).eq(encode_id(id.as_str())))
                .order_by(a("user_id"), Order::Asc)
                .order_by(a("ticket_id"), Order::Asc)
                .lock_exclusive(),
        )
        .await
        .map_err(storage)?;
    let tickets = rows
        .into_iter()
        .map(decode)
        .collect::<Result<Vec<_>, _>>()?;
    if tickets.len() != 2
        || ![(black, Player::Black), (white, Player::White)]
            .iter()
            .all(|(user, side)| {
                tickets.iter().any(|t| {
                    t.user.as_str() == *user
                        && t.state
                            == TicketState::Playing {
                                game_id: id.clone(),
                                side: *side,
                            }
                })
            })
    {
        return Err(Error::Corrupt);
    }
    let result = tx
        .execute(
            Query::update()
                .table(a("matchmaking_tickets"))
                .value(a("state"), "finished")
                .and_where(Expr::col(a("game_id")).eq(encode_id(id.as_str())))
                .and_where(Expr::col(a("state")).eq("playing")),
        )
        .await
        .map_err(storage)?;
    if result.rows_affected() != 2 {
        return Err(Error::Corrupt);
    }
    Ok(())
}
