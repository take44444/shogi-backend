use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "レート更新ジョブの実行と隔離からの復旧",
    source: trace::Source {
        path: "crates/game-application/src/rating.rs",
        needle: "pub async fn tick(&self, owner: &WorkerId) -> Result<TickOutcome, RatingWorkerError> {"
    },
    flow: &[
        "A rated terminal receipt atomically creates a durable rating job from immutable admission metadata and committed result, independently of the broker.",
        "RatingWorker acquires a fenced lease, calculates from frozen pool-specific starts and requests atomic repository application.",
        "The repository updates both players, records one application ledger, completes the job and releases only that game's reservations in one transaction.",
        "Transient failures defer with backoff; corruption or exhausted acquisition budget quarantines the still-reserved game until explicit recovery."
    ],
    guarantees: &[
        trace::Guarantee {
            id: "terminal-durable-input",
            statement: "Only a durable terminal rated game produces a rating job, atomically with the accepted receipt and outbox. Calculation uses frozen pool-specific starts and the committed winner.",
            contracts: &["terminal-job", "pure-calculation", "worker-reconciliation"]
        },
        trace::Guarantee {
            id: "zero-sum-checked-elo",
            statement: "K32 Elo treats unrated players as 1500, rounds one black delta and negates it for white, and checks signed-rating and completed-game-count arithmetic.",
            contracts: &["pure-calculation"]
        },
        trace::Guarantee {
            id: "atomic-once-ledger",
            statement: "A valid current claim updates both ratings, inserts one ledger, completes the job and releases only its own reservations atomically. Matching completed replay reconciles read-only.",
            contracts: &["atomic-application", "worker-reconciliation"]
        },
        trace::Guarantee {
            id: "fenced-retry",
            statement: "Expired or superseded claims cannot mutate results. Attempts, lease expiry and transient backoff are durable; corruption or exhausted budget quarantines without releasing reservations.",
            contracts: &[
                "claims-fences",
                "worker-reconciliation",
                "quarantine-recovery"
            ]
        },
        trace::Guarantee {
            id: "explicit-recovery",
            statement: "Explicit recovery resets a quarantined job's acquisition budget without repairing input or changing frozen starts, ratings or reservations. The worker binary exposes polling and retry-quarantined modes.",
            contracts: &["quarantine-recovery", "worker-process"]
        }
    ],
    limitations: &[
        "The worker reads durable jobs rather than broker messages. There is no automatic input repair or finite completion-time guarantee.",
        "Repository integration tests do not assert standalone worker CLI, polling or signal lifecycle.",
        "Public rating reads are mapped by user-ratings; startup schema and reader permissions are mapped by startup-schema-isolation.",
        "Source links and successful test execution establish traceable evidence, not semantic proof or complete coverage of these guarantees."
    ],
    entrypoints: &[
        trace::EntryPoint {
            name: "レートジョブのポーリングと隔離復旧 CLI",
            source: trace::Source {
                path: "apps/rating-worker-server/src/main.rs",
                needle: "async fn main() -> Result<(), Box<dyn std::error::Error>> {"
            }
        },
        trace::EntryPoint {
            name: "レートジョブの処理",
            source: trace::Source {
                path: "crates/game-application/src/rating.rs",
                needle: "pub async fn tick(&self, owner: &WorkerId) -> Result<TickOutcome, RatingWorkerError> {"
            }
        }
    ],
    surfaces: &[],
});
