use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "マッチメイキングの参加・状態取得・継続・キャンセル",
    source: trace::Source {
        path: "crates/game-application/src/matchmaking.rs",
        needle: "pub enum TicketState {"
    },
    flow: &[
        "Browser current/ticket/join/heartbeat/cancel handlers authenticate the cookie and derive the owner before issuing the corresponding GameService RPC.",
        "GameRpc validates typed owner, UUID, pool and heartbeat values, then delegates JoinMatchmaking, GetMatchmaking, GetMatchmakingTicket, HeartbeatMatchmaking and CancelMatchmaking to the durable repository.",
        "PostgreSQL transactions serialize admission, expire stale waiting state and preserve historical tickets, refusals and cancellation tombstones; pairing is traced separately."
    ],
    guarantees: &[
        trace::Guarantee {
            id: "owner-admission",
            statement: "An owner may request either supported rated pool, with at most one waiting or playing participation across pools. The server captures the admission rating and preserves the UUID retry identity.",
            contracts: &["admission", "rpc-boundary", "browser-owner"]
        },
        trace::Guarantee {
            id: "durable-history",
            statement: "Current participation and historical ticket reads are owner-scoped. Exact retries retain stored outcomes; a different pool conflicts. Refusals and cancellation before join remain durable.",
            contracts: &["admission", "reads", "cancel", "rpc-boundary"]
        },
        trace::Guarantee {
            id: "waiting-lifecycle",
            statement: "Waiting lasts 30 seconds from admission or the latest strictly newer positive heartbeat. Duplicate/older heartbeats do not extend or revive it. Cancellation ends waiting but cannot cancel an already matched game.",
            contracts: &["heartbeat", "cancel"]
        },
        trace::Guarantee {
            id: "browser-contract",
            statement: "The five browser matchmaking operations require a valid session, enforce Host and mutation Origin/same-site checks, reject caller user_id, return stable ticket/error JSON and apply no-store. OpenAPI describes all five operations.",
            contracts: &["browser-owner", "browser-response", "browser-schema"]
        }
    ],
    limitations: &[
        "gRPC accepts caller identity claims and does not authenticate users; authenticated owner derivation belongs to the browser gateway.",
        "Selected tests do not exhaust every heartbeat ordering, malformed field, no-store response class or backend failure branch.",
        "Links and passing tests do not establish semantic completeness; changes to guarantees, evidence strength and decomposition require review."
    ],
    entrypoints: &[],
    surfaces: &[
        trace::Surface::OpenApi {
            package: "realtime-gateway",
            example: "matchmaking-openapi",
            operation: "GET /matchmaking/me"
        },
        trace::Surface::OpenApi {
            package: "realtime-gateway",
            example: "matchmaking-openapi",
            operation: "GET /matchmaking/tickets/{ticket_id}"
        },
        trace::Surface::OpenApi {
            package: "realtime-gateway",
            example: "matchmaking-openapi",
            operation: "PUT /matchmaking/tickets/{ticket_id}"
        },
        trace::Surface::OpenApi {
            package: "realtime-gateway",
            example: "matchmaking-openapi",
            operation: "DELETE /matchmaking/tickets/{ticket_id}"
        },
        trace::Surface::OpenApi {
            package: "realtime-gateway",
            example: "matchmaking-openapi",
            operation: "POST /matchmaking/tickets/{ticket_id}/heartbeat"
        },
        trace::Surface::Grpc {
            path: "proto/game.proto",
            service: "GameService",
            operation: "JoinMatchmaking"
        },
        trace::Surface::Grpc {
            path: "proto/game.proto",
            service: "GameService",
            operation: "GetMatchmaking"
        },
        trace::Surface::Grpc {
            path: "proto/game.proto",
            service: "GameService",
            operation: "GetMatchmakingTicket"
        },
        trace::Surface::Grpc {
            path: "proto/game.proto",
            service: "GameService",
            operation: "HeartbeatMatchmaking"
        },
        trace::Surface::Grpc {
            path: "proto/game.proto",
            service: "GameService",
            operation: "CancelMatchmaking"
        }
    ],
});
