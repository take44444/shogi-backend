use game_application::OutboxPublisher;
use game_messaging::ValkeyEventPublisher;
use game_persistence::PostgresOutbox;
use std::time::Duration;

type Error = Box<dyn std::error::Error>;
fn main() -> Result<(), Error> {
    let database = std::env::var("DATABASE_URL")?;
    let valkey = std::env::var("VALKEY_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let runtime = tokio::runtime::Runtime::new()?;
    let mut queue = runtime.block_on(PostgresOutbox::connect(&database))?;
    let outcome = runtime.block_on(run(&mut queue, &valkey));
    drop(runtime);
    drop(queue);
    outcome
}
async fn run(queue: &mut PostgresOutbox, valkey: &str) -> Result<(), Error> {
    let transport = ValkeyEventPublisher::connect(valkey).await?;
    let mut publisher = OutboxPublisher::new(queue, transport);
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);
    eprintln!("outbox-publisher acquired exclusive PostgreSQL queue");
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
