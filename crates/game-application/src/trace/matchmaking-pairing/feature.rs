use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "レート差に基づく対戦選択と対局作成",
    source: trace::Source {
        path: "crates/game-application/src/matchmaking.rs",
        needle: "pub fn select_pair(tickets: &[Ticket], now: i64) -> Option<(&Ticket, &Ticket)> {"
    },
    flow: &[
        "The pure selector filters live waiting tickets, sorts deterministic anchors and checks both rating windows within one pool.",
        "The durable tick transaction rechecks locked tickets and ratings after lock waits, creates a rated game and changes both tickets to permanent Playing bindings together.",
        "The game-server matcher invokes tick on a one-second interval, alternates the first pool and performs a bounded batch without requiring another join."
    ],
    guarantees: &[
        trace::Guarantee {
            id: "policy",
            statement: "Pairing skips an incompatible head, uses the oldest compatible anchor and nearest opponent with age/UUID/owner tie breaks, requires distinct users and one pool, and applies both widening windows.",
            contracts: &["windows", "selection"]
        },
        trace::Guarantee {
            id: "server-ownership",
            statement: "The server chooses the game and permanent Black/White bindings. Pair formation and two ticket transitions commit together; concurrent matchers have one durable winner.",
            contracts: &["durable-tick"]
        },
        trace::Guarantee {
            id: "expiry",
            statement: "Expired tickets cannot pair, including when expiry occurs during rating-lock waits.",
            contracts: &["selection", "durable-tick"]
        },
        trace::Guarantee {
            id: "periodic-progress",
            statement: "The actual game-server runs matching periodically so a waiting pair can become eligible without another join; each batch alternates starting pool, caps attempts at 32 and stops after two empty pools or a failure.",
            contracts: &["periodic"]
        }
    ],
    limitations: &[
        "The process test proves periodic progress for its one-pool case; pure tests directly assert tie-break policy and do not prove production fairness or timing under load.",
        "Pool names do not enable clocks or time-control adjudication.",
        "Links and passing tests do not establish semantic completeness; changes to guarantees, evidence strength and decomposition require review."
    ],
    entrypoints: &[trace::EntryPoint {
        name: "待機チケットの周期的マッチング",
        source: trace::Source {
            path: "apps/game-server/src/main.rs",
            needle: "let matcher = tokio::spawn(async move {"
        }
    }],
    surfaces: &[],
});
