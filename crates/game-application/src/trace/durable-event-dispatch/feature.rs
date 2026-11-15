use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "対局イベントの永続化と順序付き配信",
    source: trace::Source {
        path: "crates/game-application/src/events.rs",
        needle: "pub async fn publish_next(&mut self) -> Result<bool, DispatchError> {"
    },
    flow: &[
        "StagedGameChange freezes canonical facts from an accepted receipt, and the PostgreSQL commit writes the receipt and outbox rows atomically.",
        "An exclusively locked PostgreSQL session supplies the earliest pending row without holding a transaction across publication.",
        "OutboxPublisher sends version-one JSON to the exact game channel and marks only the selected row after confirmed transport acceptance.",
        "An uncertain send or acknowledgement leaves a retryable identity that can be published again."
    ],
    guarantees: &[
        trace::Guarantee {
            id: "atomic-facts",
            statement: "Accepted receipts and canonical frozen outbox facts commit together. Rejection and same-request replay do not enqueue extra facts.",
            contracts: &["canonical-facts", "atomic-outbox"]
        },
        trace::Guarantee {
            id: "ordered-identity",
            statement: "Dispatch preserves queue order and stable identity (game_id, seq, kind). Uncertain send or marking can duplicate an identity without skipping pending work.",
            contracts: &["exclusive-queue", "confirmed-dispatch", "wire-format"]
        },
        trace::Guarantee {
            id: "confirmed-acceptance",
            statement: "An exact row is marked only after a nonnegative PUBLISH integer response, including zero subscribers. This confirms transport acceptance rather than subscriber delivery or durable broker storage.",
            contracts: &["confirmed-dispatch"]
        },
        trace::Guarantee {
            id: "exclusive-fail-closed",
            statement: "One PostgreSQL session owns the publisher lock. Losing that session fails closed, and no database transaction stays open at the delivery boundary.",
            contracts: &["exclusive-queue"]
        },
        trace::Guarantee {
            id: "process-dispatch",
            statement: "The publisher process owns the queue and transport, polls pending work and stops on dispatch failure or Ctrl-C.",
            contracts: &["publisher-process"]
        }
    ],
    limitations: &[
        "No mapped test traverses a real database, deployed Valkey and live subscriber together. No subscriber delivery or broker persistence is promised.",
        "Standalone process startup, restart and signal behavior remain explicit gaps even though queue-lock and adapter cancellation behavior have dedicated tests.",
        "Schema validation and database-role policy are mapped by startup-schema-isolation.",
        "Source links and successful test execution establish traceable evidence, not semantic proof or complete coverage of these guarantees."
    ],
    entrypoints: &[
        trace::EntryPoint {
            name: "永続 Outbox の一件配信",
            source: trace::Source {
                path: "crates/game-application/src/events.rs",
                needle: "pub async fn publish_next(&mut self) -> Result<bool, DispatchError> {"
            }
        },
        trace::EntryPoint {
            name: "Outbox 配信プロセス",
            source: trace::Source {
                path: "apps/outbox-publisher/src/main.rs",
                needle: "fn main() -> Result<(), Error> {"
            }
        }
    ],
    surfaces: &[],
});
