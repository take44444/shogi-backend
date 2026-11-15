use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "着手・将棋ルール・詰み・投了",
    source: trace::Source {
        path: "crates/shogi-domain/src/lib.rs",
        needle: "pub fn play(&mut self, player: Player, mv: Move) -> Result<MoveOutcome, CommandError> {"
    },
    flow: &[
        "MakeMove and Resign validate wire IDs and required expected sequence; move input is parsed as strict USI before entering the application.",
        "The actor derives the side from immutable participant bindings and stages a cloned game; domain rules apply only legal moves, and errors preserve observable state.",
        "The rules adapter generates legal moves including promotion/drop/king safety restrictions; domain logic recognizes checkmate or participant resignation and retains terminal position.",
        "Accepted candidate state enters the atomic receipt/event commit path traced in game-command-recovery; RPC conversion exposes stable acceptance and rejection statuses."
    ],
    guarantees: &[
        trace::Guarantee {
            id: "participant-input",
            statement: "Only the bound participants may mutate. The side comes from permanent bindings, moves use strict USI and malformed wire values fail before changing the game.",
            contracts: &["rpc-move", "rpc-resign", "binding", "notation"]
        },
        trace::Guarantee {
            id: "shogi-rules",
            statement: "Legal play supports movement for all ordinary/promoted pieces, blockers, capture/demotion into hand, drops, nifu, dead-rank restrictions, optional/mandatory promotion, king safety/check evasion and pawn-drop-mate rejection.",
            contracts: &["legal-moves", "domain-play"]
        },
        trace::Guarantee {
            id: "terminal",
            statement: "Checkmate ends the game with the mating side as winner; no moves without check does not cause a loss. Either side may resign off turn, making the opponent winner. Finished games reject new moves/resignations and expose no legal moves.",
            contracts: &["domain-play", "domain-resign", "rpc-errors"]
        },
        trace::Guarantee {
            id: "atomic-rejection",
            statement: "Domain rejection preserves the whole game, and the application exposes no committed position change from rejected commands. Rules-engine failure remains distinct from ordinary illegality.",
            contracts: &["legal-moves", "domain-play", "rpc-errors"]
        },
        trace::Guarantee {
            id: "adjudication-scope",
            statement: "Repetition and perpetual-check positions remain playable. There is no clock, timeout, entering-king/declaration or repetition adjudication; pool names do not turn these on.",
            contracts: &["domain-play"]
        }
    ],
    limitations: &[
        "All supported rule families have direct fixture evidence; the selected real-network test covers a representative legal sequence and resignation, not every rule or a full checkmate sequence.",
        "SFEN construction is test-only fixture support, not a public import/editor feature. There is no supported client-controlled game creation RPC.",
        "gRPC caller identities are trusted claims. Browser authentication and spoof rejection are traced in realtime-session-commands-limits.",
        "Links and passing tests do not establish semantic completeness; changes to guarantees, evidence strength and decomposition require review."
    ],
    entrypoints: &[],
    surfaces: &[
        trace::Surface::Grpc {
            path: "proto/game.proto",
            service: "GameService",
            operation: "MakeMove"
        },
        trace::Surface::Grpc {
            path: "proto/game.proto",
            service: "GameService",
            operation: "Resign"
        }
    ],
});
