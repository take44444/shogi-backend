# テストを配置する

## 配置先を選ぶ

検証する公開境界を所有する crate の `tests` に統合テストを置く。
DB や TCP を使うテストも、この所有関係で配置する。
実行ファイルの起動や停止を検証するテストは、その `apps/*/tests` に置く。
内部実装にアクセスする単体テストは、対象の `src` モジュールの近くに置く。

| 対象 | 配置例 |
|---|---|
| crate のルートにある型や関数 | `src/tests.rs` または `src/tests/mod.rs` |
| `actor.rs` の内部動作 | `src/actor/tests/mod.rs` と振る舞い別の `.rs` |
| gRPC のコマンド受付 | `tests/game_commands/retries.rs` |
| 永続化のトランザクション | `tests/repository/transaction_failures.rs` |
| HTTP の公開レコード | `tests/public_record_detail/application_forwarding.rs` |
| コンパイル可否で検査する公開インターフェース | リポジトリルートの `tests/interface_checks/*.rs` |

既存の Python、コンパイラ fixture、ブラウザーテストは、それぞれのツールの近くに置く。

## 機能と振る舞いで分ける

ディレクトリを公開機能、ファイルを観測できる振る舞いで命名する。
Rust のモジュール名に合わせ、ディレクトリ名には `snake_case` を使う。

```text
crates/game-grpc/tests/
├── tcp.rs
├── game_commands/
│   ├── mod.rs
│   ├── concurrency.rs
│   ├── restart_recovery.rs
│   └── retries.rs
├── game_play/
│   ├── mod.rs
│   ├── input_validation.rs
│   ├── moves.rs
│   ├── permissions.rs
│   └── rpc_status.rs
├── matchmaking/
│   └── cancellation.rs
└── support/
    └── server.rs
```

小さいテスト群は一つの `.rs` にまとめる。
`feature/scenario/tests.rs` のような、一つのケースファイルを包むための階層は作らない。
Cargo が `tests` 直下の `.rs` をテストターゲットとして扱うため、同じ実行対象に属するケースは機能ディレクトリの下に置く。
単独の実行対象なら、`apps/game-server/tests/matchmaking.rs` のように入口へ直接記述できる。

## モジュールの読み込みと補助処理を分ける

Cargo のテスト入口、または機能ごとの `mod.rs` からケースを読み込む。
`support.rs` や `support/*.rs` には、起動、fixture 作成、リクエスト構築などの補助処理を置く。
support からケースを読み込まない。
単独のケースにしか必要ない短いセットアップは、そのケースの近くに置ける。

実行対象と同じ名前のディレクトリを使う場合は、入口から読み込み先を明示する。
例えば `tests/repository.rs` は `#[path = "repository/mod.rs"] mod repository;` とする。
ヘルパーの可視性は、必要なテスト群から使える範囲に限定する。

`player-http/tests/support/router.rs` は、ライブラリ内テストと統合テストが共有するヘルパーである。
ライブラリ側は `#[cfg(test)]` と明示的な `#[path]` で読み込み、製品 API に公開しない。

SQL などの入力データは `fixtures` に置く。
移動するときは `include_str!` と `include_bytes!` の参照先も更新する。

## 契約とテスト対象を記録する

振る舞いの条件は Rustdoc とテストの assertion に記録する。
テストの対象 API は `#[traceability::targets(...)]` で指定する。
ディレクトリ名から API との対応を推測しない。
[コンポーネント境界とテスト対象の生成](check-traceability.md)に従って対応を確認する。

## 配置と移動結果を検査する

構造の検査を実行する。

```sh
python3 scripts/test-check-test-layout.py
python3 scripts/check-test-layout.py
```

大きな移動では、変更前の checkout からテストのスナップショットを保存する。
以下の `/tmp/shogi-before` は変更前の checkout、現在のディレクトリは変更後の checkout とする。

```sh
python3 scripts/check-test-layout.py snapshot --root /tmp/shogi-before --output /tmp/tests-before.json
python3 scripts/check-test-layout.py compare --baseline /tmp/tests-before.json
```

比較ではテスト関数名、属性、本文、fixture の内容を保持する。
ファイルとモジュールの移動、リテラル外の整形、同じ内容を参照する `include_str!` の相対パス変更を許容する。
この比較は名前解決やヘルパーの動作を検証しないため、コンパイルと実際のテスト実行も行う。

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
scripts/test-postgres.sh cargo test --locked --workspace
python3 scripts/check-contract-interfaces.py
python3 scripts/traceability.py check
```

実行対象を移動した場合は、Cargo が生成したテスト一覧も比較する。
`inventory` は Cargo の JSON 出力に記録された実行ファイルを `--list` で起動する。
信頼するローカルビルドの出力を使う。

```sh
cargo test --locked --workspace --no-run --message-format=json > /tmp/test-artifacts.jsonl
python3 scripts/check-test-layout.py inventory --artifacts /tmp/test-artifacts.jsonl --output /tmp/test-inventory.json
```

変更前後でそれぞれ JSON 出力を保存し、変更後の実行ファイルが変更前のビルドで上書きされないようにする。
`compare-inventory --artifacts /tmp/test-artifacts.jsonl --baseline /tmp/test-inventory-before.json` で比較する。
意図したターゲット名の変更だけを `--rename-target package:old=new` で指定する。
テスト名は末尾の関数名で比較し、所属 package、実行対象、重複件数、ignore 指定を維持する。
