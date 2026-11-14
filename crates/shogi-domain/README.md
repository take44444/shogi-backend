# shogi-domain の参照仕様

`Game` は標準の将棋対局を開始する。
呼び出し元は、ドメインが定義する手番、範囲を制限した升、着手、駒、結果、局面スナップショットを使う。
先手が最初に指す。段は絶対座標で、後手側の最奥が 1、先手側の最奥が 9 となる。
USI はこれらの段を `a` から `i` で表す。

## 対局 API の使用例

```rust
use shogi_domain::{Game, Player, Square, Piece, PieceKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
	Ok(())
}
```

## ルールと拒否する操作

ルールは、全駒の移動、駒による経路の遮断、駒取りと持ち駒に戻す際の成りの解除、駒打ち、二歩、
行き所のない駒の制限、任意の成りと強制成り、王の安全、王手の回避、打ち歩詰めの禁止を扱う。
王手されていて合法手がない側は詰みで負ける。
王手されておらず合法手がない側は、自動的には負けにならない。
どちらの側も手番に関係なく投了できる。

不正な着手は拒否し、対局は継続する。
拒否した操作は、盤面、持ち駒、手番、王手状態、結果、合法手をすべて保持する。
終了済みの対局は、手番や着手の合法性を調べる前に操作を拒否する。
容量やカウンターの上限による失敗は、`IllegalMove` とは別の `RulesFailure` を返す。

USI 解析は、升、盤上の着手、成っていない駒の打ち手について厳密な構文だけを受け付ける。
局面上の合法性ではなく、構文を検証する。
公開の局面編集機能、SFEN 読み込み機能、復元用コンストラクターはない。

このクレートは千日手、連続王手の千日手、入玉、宣言勝ち、持ち時間を判定しない。
同じ局面を繰り返しても対局を継続できる。
タイマー、永続化、ネットワークプロトコル、アクター、操作の重複排除は提供しない。

## ルールエンジンとの境界

非公開の `adapter` モジュールがルール上の局面を保持し、rsshogi をインポートする唯一のモジュールとなる。
`Game` が保持するのはその局面と終局結果だけで、局面スナップショットは必要なときに生成する。
アダプターは初期化、合法手生成、失敗しても状態を変えない原子的な着手適用、スナップショット変換を実装する。
公開のドメイン型と操作の意味は、ルールライブラリーに依存しない。
テスト専用の SFEN 補助関数は非公開で、本番ビルドには含めない。

## テスト範囲

ドメインのテストは、着手の合法性、終局結果、拒否した操作の後の状態保持を対象とする。
実行コマンドは[ワークスペースの検証手順](../../README.md#ワークスペースを検証する)に記述する。
