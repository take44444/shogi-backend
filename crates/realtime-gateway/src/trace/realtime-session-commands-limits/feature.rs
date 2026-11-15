use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "WebSocket の認証・対局操作・資源制限",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "async fn upgrade("
    },
    flow: &[
        "Upgrade validates exact browser Origin/Host, optional same-site metadata and one supported shogi.realtime.v1 offer, then checks the session cookie with the user service and game access with GameService.",
        "The reader validates strict move/resign JSON and canonical decimal sequence; one in-flight permit and bounded queues limit command work. The gateway supplies the authenticated user to the upstream command.",
        "The command worker and writer recheck session/access before actions and outgoing frames; acceptance/errors become stable browser frames and uncertain outcomes require exact retry.",
        "Connection/user permits, input byte/rate limits, snapshot/buffer bounds and timed upstream/output operations constrain connection work. Gateway shutdown rejects upgrades and closes active sockets."
    ],
    guarantees: &[
        trace::Guarantee {
            id: "handshake",
            statement: "Upgrade requires exact Origin/Host, optional same-origin fetch metadata, exactly one supported subprotocol offer and one valid session cookie. Session authentication and game access precede upgrade.",
            contracts: &["browser-headers", "session-auth", "upgrade-permits"]
        },
        trace::Guarantee {
            id: "commands",
            statement: "Only strict move/resign commands are accepted; no caller user/side claim is accepted. expected_seq is a canonical decimal string through i64::MAX and the gateway supplies authenticated identity. Accepted and rejected commands use stable response codes; uncertain outcomes become outcome_unknown.",
            contracts: &["command-input", "command-response", "live-checks"]
        },
        trace::Guarantee {
            id: "live-authorization",
            statement: "Session and game access are rechecked before command dispatch and each outgoing frame. Logout/expiry or privacy loss closes the connection before further permitted delivery.",
            contracts: &["session-auth", "live-checks", "outgoing"]
        },
        trace::Guarantee {
            id: "resource-limits",
            statement: "Total and per-user connections, input frame/message bytes/rate, one in-flight command, bounded notification/output queues, snapshot size and upstream/output deadlines constrain connection work.",
            contracts: &["upgrade-permits", "command-input", "outgoing"]
        },
        trace::Guarantee {
            id: "shutdown",
            statement: "Gateway shutdown prevents new upgrades and closes existing connections, including shutdown before the first connection. Capacity is released after disconnect.",
            contracts: &["shutdown-contract", "upgrade-permits"]
        }
    ],
    limitations: &[
        "The gateway authenticates browser identity; the upstream trusted gRPC service itself accepts claims. Session/access checks bound disclosure at checked operations, not a globally atomic cross-service authorization snapshot.",
        "Resource bounds cover configured queues/permits and deadlines; selected tests do not prove every overload or timeout branch. There is no separate handshake/input semaphore beyond the implemented connection and in-flight command permits.",
        "Executable startup/configuration/signal behavior is owned by the runtime feature record and is not established by these library router tests.",
        "Links and passing tests do not establish semantic completeness; changes to guarantees, evidence strength and decomposition require review."
    ],
    entrypoints: &[
        trace::EntryPoint {
            name: "認証済み WebSocket 接続",
            source: trace::Source {
                path: "crates/realtime-gateway/src/lib.rs",
                needle: ".route(\"/ws/games/{game_id}\", get(upgrade))"
            }
        },
        trace::EntryPoint {
            name: "WebSocket move と resign の入力",
            source: trace::Source {
                path: "crates/realtime-gateway/src/game.rs",
                needle: "enum WireCommand {"
            }
        }
    ],
    surfaces: &[],
});
