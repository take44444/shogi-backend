use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "終了済み公開対局の履歴取得",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "async fn detail(State(app): State<App>, path: Result<Path<String>, PathRejection>) -> Response {"
    },
    flow: &[
        "The HTTP adapter validates the game ID and calls GameRecords::detail under the shared request limit.",
        "The PostgreSQL reader reads public completed metadata and its entire bounded journal in one repeatable-read snapshot.",
        "Typed metadata and contiguous public operations become a public DTO; corruption produces an error rather than partial history."
    ],
    guarantees: &[
        trace::Guarantee {
            id: "public-detail",
            statement: "Anonymous detail returns public completed metadata and contiguous ordered public operations, excluding command receipts, retry identifiers and internal state.",
            contracts: &[
                "http-boundary",
                "public-dto",
                "metadata-invariants",
                "snapshot-history"
            ]
        },
        trace::Guarantee {
            id: "hidden-indistinguishable",
            statement: "Missing, ongoing and hidden games return not_found, while malformed IDs are rejected before storage.",
            contracts: &["http-boundary", "snapshot-history"]
        },
        trace::Guarantee {
            id: "complete-bounded-history",
            statement: "History is accepted only after structural validation and at most 4096 operations. Corruption returns generic 500 without partial history, excess history returns 422, and storage unavailability returns 503.",
            contracts: &["http-boundary", "metadata-invariants", "snapshot-history"]
        },
        trace::Guarantee {
            id: "one-view",
            statement: "Metadata and journal are read from one database view even if another connection changes both between reads.",
            contracts: &["application-forwarding", "snapshot-history"]
        }
    ],
    limitations: &[
        "No mapped test starts a player-server TCP listener. No guarantee is made that structurally readable history consists of legal shogi play.",
        "Shared HTTP byte/admission bounds and reader role/timeouts are mapped in public-record-search and startup-schema-isolation.",
        "Source links and successful test execution establish traceable evidence, not semantic proof or complete coverage of these guarantees."
    ],
    entrypoints: &[],
    surfaces: &[trace::Surface::OpenApi {
        package: "player-http",
        example: "player-openapi",
        operation: "GET /game-records/{id}"
    }],
});
