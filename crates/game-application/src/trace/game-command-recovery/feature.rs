use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "対局操作の再試行・永続化・復旧",
    source: trace::Source {
        path: "crates/game-application/src/actor.rs",
        needle: "async fn apply(&mut self, pending: MutationRequest) -> Result<Acceptance, ApplicationError> {"
    },
    flow: &[
        "The per-game actor refreshes durable history and checks participant membership, exact accepted request replay, then expected sequence before staging a cloned legal candidate.",
        "StagedGameChange freezes the acceptance and canonical event batch; commit_change performs version comparison and persists metadata, one receipt and those events atomically.",
        "After an uncertain/conflicting commit the actor reloads durable receipts, returns the original acceptance when present and otherwise reports conflict/unavailability while fencing uncertain local state.",
        "Caller cancellation after enqueue leaves the actor transaction running; restart reconstructs receipts, and dropping the final service sender lets queued commands drain."
    ],
    guarantees: &[
        trace::Guarantee {
            id: "exact-retry",
            statement: "Only acceptance reserves a per-game request identity. After participant checks, an exact typed action/user/original expected-sequence retry returns the original acceptance before stale/finished checks, even after restart; changed accepted content conflicts.",
            contracts: &["replay", "actor-apply"]
        },
        trace::Guarantee {
            id: "sequence",
            statement: "Each new committed command advances sequence once through i64::MAX. Per-game actor processing and repository CAS serialize competing service instances and refresh losing state.",
            contracts: &["actor-apply", "durable-commit", "sequence-cap"]
        },
        trace::Guarantee {
            id: "atomic-events",
            statement: "Candidate position becomes visible only after confirmed commit. Metadata, receipt and supplied canonical events commit together: move emits MoveCommitted, mate appends GameFinished at the same sequence, resignation emits GameFinished. Replay/rejection/restore emit nothing new.",
            contracts: &["actor-apply", "durable-commit", "canonical-events"]
        },
        trace::Guarantee {
            id: "uncertain-recovery",
            statement: "An error may follow a durable commit. The actor reconciles a matching stored receipt or invalidates its state until a later successful durable refresh; failed reads never authorize continued cached mutation.",
            contracts: &["actor-apply", "reconciliation"]
        },
        trace::Guarantee {
            id: "cancellation",
            statement: "The 64-slot per-game mailbox bounds queued commands. Cancellation after enqueue does not undo execution; cancellation before enqueue does not execute, and closed senders allow queued work to drain.",
            contracts: &["mailbox"]
        }
    ],
    limitations: &[
        "Successful representative fault/concurrency tests do not prove all crash schedules or durable infrastructure guarantees.",
        "Game command trace owns canonical event production and atomic enqueue; publication, subscriber delivery and rating application are separate feature records.",
        "Actor/mailbox fairness, total memory for known-game registries and an upper bound on waiting callers are not promised.",
        "Links and passing tests do not establish semantic completeness; changes to guarantees, evidence strength and decomposition require review."
    ],
    entrypoints: &[trace::EntryPoint {
        name: "対局アクターと操作の復旧",
        source: trace::Source {
            path: "crates/game-application/src/actor.rs",
            needle: "struct Actor {"
        }
    }],
    surfaces: &[],
});
