# shogi-domain

`Game` starts a standard shogi game. Callers use domain-owned players, bounded
squares, moves, pieces, results, and position snapshots. Black moves first.
Ranks are absolute, with 1 at White's back rank and 9 at Black's back rank.
USI uses `a` through `i` for those ranks.

```rust
use shogi_domain::{Game, Player, Square, Piece, PieceKind};

let mut game = Game::new();
assert_eq!(game.legal_moves().len(), 30);
let outcome = game.play(Player::Black, "7g7f".parse()?)?;
assert_eq!(game.turn(), Player::White);
assert!(!outcome.in_check);
assert_eq!(game.position().piece_at(Square::new(7, 6)?),
    Some(Piece { owner: Player::Black, kind: PieceKind::Pawn }));
game.play(Player::White, "3c3d".parse()?)?;
let result = game.resign(Player::Black)?;
assert_eq!(result.winner, Player::White);
assert!(game.legal_moves().is_empty());
# Ok::<(), Box<dyn std::error::Error>>(())
```

The rules cover all piece movements, blocking, capture and demotion into hand,
drops, nifu, dead-rank restrictions, optional and mandatory promotion, king
safety, check evasion, and pawn-drop mate rejection. A checked player with no
legal moves loses by checkmate. A player without legal moves who is not checked
does not lose automatically. Either player may resign, even off turn.

An illegal move is rejected and play continues. Every rejected command preserves
the board, hands, turn, check status, result, and legal moves. Finished games
reject commands before checking turn or move legality. Capacity or counter
failures return `RulesFailure`, separately from `IllegalMove`.

USI parsing accepts only exact square, board-move, and unpromoted drop syntax.
It checks syntax rather than position legality. There is no public position
editor, SFEN importer, or restore constructor.

This crate does not adjudicate repetition, perpetual check, entering king,
declaration, or time controls. Repeated positions remain playable. It provides
no timers, persistence, network protocol, actor, or command deduplication.

The private `adapter` module owns the rules position and is the only module
that imports rsshogi. `Game` retains only that position and its terminal result;
position snapshots are derived on demand. To replace the rules library, replace
the adapter's construction, move generation, fallible atomic application, and
snapshot conversion. Keep domain types and command semantics unchanged. Run the
public example and the curated rules and atomicity tests against the replacement.
The test-only SFEN helper is private and is absent from production builds.
