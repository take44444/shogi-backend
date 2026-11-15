use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "終了済み公開対局の検索",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "async fn search("
    },
    flow: &[
        "The HTTP adapter decodes and validates filters and the cursor before acquiring shared request capacity.",
        "GameRecords forwards the typed Search to the read repository, which selects public completed metadata with keyset pagination.",
        "The HTTP adapter serializes metadata and the next cursor through the bounded no-store response helper."
    ],
    guarantees: &[
        trace::Guarantee {
            id: "public-filtered-metadata",
            statement: "Anonymous search returns metadata only from currently public completed games and combines supplied filters with AND.",
            contracts: &["query-boundary", "application-forwarding", "public-keysets"]
        },
        trace::Guarantee {
            id: "bounded-input",
            statement: "Search requires a nonnegative increasing UTC microsecond interval with inclusive start and exclusive end of at most 366 days, a limit of 1 through 100 with default 30, and valid typed filters without unknown or repeated fields. Invalid inputs return 400 before storage.",
            contracts: &["query-boundary", "bounds-cursor"]
        },
        trace::Guarantee {
            id: "seek-pages",
            statement: "Results descend by finish time and then binary game ID. The cursor binds filters while allowing a changed page limit, and subsequent pages recheck visibility.",
            contracts: &["bounds-cursor", "public-keysets"]
        },
        trace::Guarantee {
            id: "resource-response-policy",
            statement: "Handled responses use generic errors and no-store. The shared HTTP adapter admits at most 16 active requests and rejects serialized successes above one MiB.",
            contracts: &["http-resource-policy"]
        }
    ],
    limitations: &[
        "Pages do not promise one fixed database snapshot across requests. Cursors are filter-bound values, not authenticated tokens.",
        "No mapped test starts a real player-server TCP listener. Raw framework fallbacks are outside the handled-response no-store guarantee.",
        "Reader connection limits, database timeouts and database permissions are mapped by startup-schema-isolation.",
        "Source links and successful test execution establish traceable evidence, not semantic proof or complete coverage of these guarantees."
    ],
    entrypoints: &[],
    surfaces: &[trace::Surface::OpenApi {
        package: "player-http",
        example: "player-openapi",
        operation: "GET /game-records"
    }],
});
