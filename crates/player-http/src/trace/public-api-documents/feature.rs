use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "公開 API 仕様の取得とオフライン生成",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "pub fn openapi() -> utoipa::openapi::OpenApi {"
    },
    flow: &[
        "User and player OpenApiRouter registrations produce offline documents and their anonymous /openapi.json responses.",
        "The gateway exports a separate offline matchmaking OpenAPI document with owner-scoped routes and cookie alternatives.",
        "Example binaries serialize these library documents; document generation does not start the services."
    ],
    guarantees: &[
        trace::Guarantee {
            id: "offline-runtime-equality",
            statement: "User and player expose anonymous /openapi.json equal to offline generation without repository access or user identity-provider calls.",
            contracts: &["player-runtime", "user-runtime"]
        },
        trace::Guarantee {
            id: "player-schema",
            statement: "Player OpenAPI identifies its service and describes its routes, query/error contracts and public response schemas.",
            contracts: &["contract-player-schema"]
        },
        trace::Guarantee {
            id: "user-schema",
            statement: "User OpenAPI identifies its service and describes its routes, cookie alternatives, DTOs, error media and redirect headers.",
            contracts: &["contract-user-schema"]
        },
        trace::Guarantee {
            id: "matchmaking-document",
            statement: "The gateway offline document includes the five owner-scoped matchmaking operations and production/development session-cookie alternatives.",
            contracts: &["gateway-schema"]
        },
        trace::Guarantee {
            id: "document-commands",
            statement: "The three example commands print library OpenAPI JSON without requiring service startup.",
            contracts: &["example-exports"]
        }
    ],
    limitations: &[
        "Generated schema assertions and in-process responses do not prove documentation publication or real example-command stdout.",
        "Only user and player have runtime document endpoints. The gateway document covers matchmaking; the WebSocket protocol is outside that OpenAPI.",
        "Source links and successful test execution establish traceable evidence, not semantic proof or complete coverage of these guarantees."
    ],
    entrypoints: &[trace::EntryPoint {
        name: "オフラインのマッチメイキング API 仕様",
        source: trace::Source {
            path: "crates/realtime-gateway/examples/matchmaking-openapi.rs",
            needle: "fn main()"
        }
    }],
    surfaces: &[
        trace::Surface::OpenApi {
            package: "user-http",
            example: "user-openapi",
            operation: "GET /openapi.json"
        },
        trace::Surface::OpenApi {
            package: "player-http",
            example: "player-openapi",
            operation: "GET /openapi.json"
        }
    ],
});
