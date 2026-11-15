use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "対局の初期局面と確定イベントのリアルタイム配信",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "async fn run_connection("
    },
    flow: &[
        "After authenticated upgrade and access checks, run_connection establishes the Valkey subscription before requesting the viewer snapshot and buffers arriving committed notifications.",
        "The snapshot is queued first; a per-connection cursor ignores pre-boundary/duplicate events and accepts only consistent contiguous committed events, including the same-sequence checkmate pair.",
        "The writer rechecks session/game access before each outgoing frame. Browser events omit internal command user/request IDs; gaps, malformed subscription input or overflow terminate the connection.",
        "A reconnect obtains a fresh snapshot. A client may retry the original command identity to recover acceptance, while there is no durable browser event replay."
    ],
    guarantees: &[
        trace::Guarantee {
            id: "bootstrap",
            statement: "A connection subscribes before freezing the viewer snapshot, sends snapshot first and then delivers later committed events without losing the subscribe/snapshot race.",
            contracts: &["bootstrap-contract", "snapshot-frame"]
        },
        trace::Guarantee {
            id: "committed-feed",
            statement: "Participants and authenticated public spectators receive committed moves and final results. Event JSON exposes game/sequence/action/result and omits internal user_id/request_id.",
            contracts: &["event-frame", "bootstrap-contract"]
        },
        trace::Guarantee {
            id: "cursor-consistency",
            statement: "Exact duplicates and events at/before snapshot boundary are ignored. Gaps or conflicting same-sequence facts require a new connection; a matching mate move and finish share one sequence and deliver in order.",
            contracts: &["cursor", "subscription-pump"]
        },
        trace::Guarantee {
            id: "reconnect",
            statement: "Reconnect uses a fresh current snapshot and can replay the original command acceptance without creating a second move/event. No durable browser replay is provided.",
            contracts: &["bootstrap-contract", "cursor"]
        }
    ],
    limitations: &[
        "Notifications are transient. Browser reconnect receives a fresh snapshot, and acceptance recovery uses exact command retry; no durable client event replay is promised.",
        "A terminal connection may close before resync_required reaches the peer, as allowed by the shared close assertion.",
        "Outbox publication/subscriber protocol internals belong to separate messaging feature records; this graph traces consumption into browser behavior.",
        "Links and passing tests do not establish semantic completeness; changes to guarantees, evidence strength and decomposition require review."
    ],
    entrypoints: &[trace::EntryPoint {
        name: "スナップショットと通知の接続処理",
        source: trace::Source {
            path: "crates/realtime-gateway/src/lib.rs",
            needle: "async fn run_connection("
        }
    }],
    surfaces: &[],
});
