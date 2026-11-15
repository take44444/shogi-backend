use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "対局スナップショットと閲覧制御",
    source: trace::Source {
        path: "crates/game-application/src/lib.rs",
        needle: "pub async fn snapshot_for_viewer("
    },
    flow: &[
        "GetSnapshot dispatches to the per-game actor, which reads the durable version and reconstructs changed or invalid state from validated receipts.",
        "GetSnapshotForViewer reads access metadata and history coherently; CheckViewAccess reads current access metadata without requesting the journal.",
        "GameRpc converts the owned application snapshot into board pieces, all hand counters, legal USI moves, permanent participants, turn/check/result and sequence; the browser gateway authenticates viewer claims separately."
    ],
    guarantees: &[
        trace::Guarantee {
            id: "snapshot-shape",
            statement: "A successful trusted snapshot returns durable sequence, permanent participants, an owned board and 14 hand counters, turn/check/result and legal moves; finished positions are retained with no legal moves.",
            contracts: &["snapshot-refresh", "owned-position", "grpc-shape"]
        },
        trace::Guarantee {
            id: "viewer-policy",
            statement: "Public games permit authenticated spectators through the gateway. Private games permit participants. Viewer snapshot/access RPCs hide inaccessible and missing games as NotFound.",
            contracts: &["viewer-read", "access-policy", "grpc-viewer"]
        },
        trace::Guarantee {
            id: "recovery",
            statement: "Snapshots refresh durable version, fail closed on read failure and reject corrupt history/metadata; restart restores ongoing and finished snapshots.",
            contracts: &["snapshot-refresh", "restore", "durable-read"]
        }
    ],
    limitations: &[
        "Trusted gRPC caller user/viewer IDs are claims; authentication belongs to the gateway. GetSnapshot is an internal trusted read.",
        "No independent query-count or concurrent privacy/history assertion covers every viewer-read implementation property.",
        "Links and passing tests do not establish semantic completeness; changes to guarantees, evidence strength and decomposition require review."
    ],
    entrypoints: &[],
    surfaces: &[
        trace::Surface::Grpc {
            path: "proto/game.proto",
            service: "GameService",
            operation: "GetSnapshot"
        },
        trace::Surface::Grpc {
            path: "proto/game.proto",
            service: "GameService",
            operation: "GetSnapshotForViewer"
        },
        trace::Surface::Grpc {
            path: "proto/game.proto",
            service: "GameService",
            operation: "CheckViewAccess"
        }
    ],
});
