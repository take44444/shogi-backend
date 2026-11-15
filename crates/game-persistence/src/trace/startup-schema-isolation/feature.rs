use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "サービスの起動設定・スキーマ互換性・DB 分離",
    source: trace::Source {
        path: "crates/game-persistence/src/schema.rs",
        needle: "pub(crate) async fn initialize(db: &DatabaseConnection) -> Result<(), SchemaError> {"
    },
    flow: &[
        "Executable configuration selects account/game databases, browser origins and service endpoints before application startup.",
        "Account persistence initializes or validates its own schema; game writers and workers initialize empty game databases or validate schema version seven.",
        "The public reader requires an existing compatible game schema and uses a bounded read-only pool under separate SELECT-only credentials.",
        "Compose and initialization SQL provide separate account storage and a credential-free gateway; runtime deployment checks are a separate non-Cargo suite."
    ],
    guarantees: &[
        trace::Guarantee {
            id: "browser-config",
            statement: "BrowserConfig requires a clean HTTPS origin, with HTTP available only through explicit loopback development mode, and derives the exact Google callback URL.",
            contracts: &["contract-browser-config"]
        },
        trace::Guarantee {
            id: "account-schema",
            statement: "Account persistence initializes an empty database, preserves compatible schema/data, and fails closed on tested incompatible schema and ledger definitions.",
            contracts: &["contract-account-schema"]
        },
        trace::Guarantee {
            id: "game-schema",
            statement: "Game startup initializes schema version seven only in an empty database and rejects incompatible required definitions or version without migration or deletion of tested schema/data.",
            contracts: &["contract-game-schema"]
        },
        trace::Guarantee {
            id: "bounded-public-reader",
            statement: "The public reader creates no schema, requires version seven, admits four database connections with two-second acquisition/statement/lock limits, and relies on a separate SELECT-only role restricted to public tables.",
            contracts: &["reader-boundary", "reader-permissions"]
        },
        trace::Guarantee {
            id: "deployment-process-config",
            statement: "Configured services separate account and game storage, give the public reader separate credentials, and give the gateway no database credentials. Account startup validates configuration, connects storage and discovers its provider before listening; game and reader startup parse their addresses and connect compatible storage before serving.",
            contracts: &[
                "process-config",
                "account-environment",
                "game-process",
                "reader-process"
            ]
        }
    ],
    limitations: &[
        "Deployment topology and reader database isolation do have existing automated checks in python3 scripts/test-deployment.py. This trace graph maps only Cargo tests; it does not execute or replace that Docker-based command.",
        "There is no application health endpoint. Compose PostgreSQL/Valkey probes do not establish application readiness.",
        "CLI/environment parsing, external Google availability, startup failure propagation and graceful signal/close behavior across all six application binaries remain source-level contracts with explicit runtime test gaps.",
        "Runtime ownership/configuration for event and rating loops is also documented by durable-event-dispatch and rating-application. No mapped startup test guarantees a process can always connect to its dependencies.",
        "Source links and successful test execution establish traceable evidence, not semantic proof or complete coverage of these guarantees."
    ],
    entrypoints: &[
        trace::EntryPoint {
            name: "対局 DB の初期化とスキーマ検査",
            source: trace::Source {
                path: "crates/game-persistence/src/schema.rs",
                needle: "pub(crate) async fn initialize(db: &DatabaseConnection) -> Result<(), SchemaError> {"
            }
        },
        trace::EntryPoint {
            name: "公開データ読み取り接続",
            source: trace::Source {
                path: "crates/game-record-persistence/src/lib.rs",
                needle: "pub async fn connect(url: &str) -> Result<Self, Error> {"
            }
        },
        trace::EntryPoint {
            name: "ユーザー DB の初期化とスキーマ検査",
            source: trace::Source {
                path: "crates/user-persistence/src/lib.rs",
                needle: "pub async fn connect(url: &str) -> Result<Self, Error> {"
            }
        },
        trace::EntryPoint {
            name: "ユーザーサービス起動",
            source: trace::Source {
                path: "apps/user-server/src/main.rs",
                needle: "async fn main() -> Result<(), Box<dyn std::error::Error>> {"
            }
        },
        trace::EntryPoint {
            name: "公開データサービス起動",
            source: trace::Source {
                path: "apps/player-server/src/main.rs",
                needle: "async fn main() -> Result<(), Box<dyn std::error::Error>> {"
            }
        },
        trace::EntryPoint {
            name: "リアルタイムゲートウェイ起動",
            source: trace::Source {
                path: "apps/realtime-gateway/src/main.rs",
                needle: "async fn main() -> Result<(), Box<dyn std::error::Error>> {"
            }
        }
    ],
    surfaces: &[],
});
