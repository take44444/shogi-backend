# 移行中に失敗した検査と解消方法

最終検査の成功だけで移行経過を説明しないため、途中の失敗も記録する。
以下は、この作業の tool output と修正後の実コードを照合した記録である。

## 最初の移行スクリプトが source anchor を解決できなかった

最初の実行は次のエラーで終了した。

```text
ValueError: anchor is not unique: crates/game-application/src/lib.rs: pub trait GameRepository: Send + Sync {
```

原因は、trait の定義元を `lib.rs` と仮定したことだった。
実際の定義元は `crates/game-application/src/repository.rs` である。
移動処理を始める前の source 検査で止まったため、定義元を修正してからスクリプトを再実行した。
移行スクリプト自体は作業用の成果物とし、製品の収集処理や互換経路として残していない。
元テストの保全は、コミットした履歴スナップショットと移行検査で再確認できる。

## 最初の collector Clippy 検査が失敗した

WebSocket の route を AST で照合する追加処理に、次の診断が三件出た。
検査コマンドの終了コードは 101 だった。

```text
error: this `if` statement can be collapsed
   --> src/main.rs:762:25
= note: `-D clippy::collapsible-if` implied by `-D warnings`
```

入れ子の `if let` と条件を let chain にまとめ、同じ AST 条件を維持した。
lint suppression は追加していない。
最終ログで同名ファイルを上書きしたが、元の診断は会話の tool output にも残っており、それを再確認した。
修正後は次の検査が終了コード 0 で通った。

```sh
CARGO_TARGET_DIR=target cargo clippy --locked --manifest-path tools/traceability/Cargo.toml --all-targets -- -D warnings
```

collector の fixture 44 件も通り、実際のゲートウェイの upgrade route と二つの wire command を収集できる。

## 関数 tokens の比較が書式変更を検出した

最初の比較は `publication_timeout_leaves_the_outbox_pending` 一件の不一致で終了した。
rustfmt が `tokio::spawn` の単一引数末尾の省略可能なカンマを削除したことが原因だった。
検査を単なる文字列のカンマ除去へ変更せず、Rust の構文を理解する rustfmt で両方の tokens を正規化してから比較する方式にした。

関数呼出の省略可能な末尾カンマは同じ fingerprint になることと、タプルの要素数を変えるカンマは異なる fingerprint になることを検査している。
実際の assertion の変更、欠落、記録していない package 変更も引き続き拒否する。
最終比較は、元の 210 テストの保全と二件の追加を確認した。
