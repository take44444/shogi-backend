use game_application::OutboxPublisher;
use game_messaging::NatsEventPublisher;
use game_persistence::SqliteOutbox;
use std::time::Duration;

type Error = Box<dyn std::error::Error>;
fn main() -> Result<(), Error> {
    let database = std::env::var("DATABASE_URL")?;
    let nats = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".into());
    let runtime = tokio::runtime::Runtime::new()?;
    let mut queue = runtime.block_on(SqliteOutbox::connect(&database))?;
    let outcome = runtime.block_on(run(&mut queue, &nats));
    drop(runtime);
    drop(queue);
    outcome
}
async fn run(queue: &mut SqliteOutbox, nats: &str) -> Result<(), Error> {
    let transport = NatsEventPublisher::connect(nats).await?;
    let mut publisher = OutboxPublisher::new(queue, transport);
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);
    eprintln!("outbox-publisher acquired exclusive SQLite queue");
    let mut delay = Duration::ZERO;
    loop {
        tokio::select! { _ = &mut shutdown => return Ok(()), _ = tokio::time::sleep(delay) => {} }
        delay = if publisher.publish_next().await? {
            Duration::ZERO
        } else {
            Duration::from_millis(100)
        };
    }
}
