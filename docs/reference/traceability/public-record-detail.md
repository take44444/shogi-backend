# 終了済み公開対局の履歴取得

このページは `python3 scripts/traceability.py render` で生成する。

元データは [crates/player-http/src/trace/public-record-detail/feature.rs](../../../crates/player-http/src/trace/public-record-detail/feature.rs)。
[全機能の一覧](../feature-traceability.md)。

## 対象機能

| ID | 機能 | 仕様 |
| --- | --- | --- |
| public-record-detail | 終了済み公開対局の履歴取得 | [crates/player-http/src/lib.rs](../../../crates/player-http/src/lib.rs#L164) |

## 保証の成立条件と構成

1. The HTTP adapter validates the game ID and calls GameRecords::detail under the shared request limit.
2. The PostgreSQL reader reads public completed metadata and its entire bounded journal in one repeatable-read snapshot.
3. Typed metadata and contiguous public operations become a public DTO; corruption produces an error rather than partial history.

以下の保証には、各行に示す契約が必要となる。テストの assertions は記載した事例の範囲に限られる。

| ID | アプリケーションの保証と条件 | 必要な契約 |
| --- | --- | --- |
| complete-bounded-history | History is accepted only after structural validation and at most 4096 operations. Corruption returns generic 500 without partial history, excess history returns 422, and storage unavailability returns 503. | [http-boundary](#contract-http-boundary), [metadata-invariants](#contract-metadata-invariants), [snapshot-history](#contract-snapshot-history) |
| hidden-indistinguishable | Missing, ongoing and hidden games return not_found, while malformed IDs are rejected before storage. | [http-boundary](#contract-http-boundary), [snapshot-history](#contract-snapshot-history) |
| one-view | Metadata and journal are read from one database view even if another connection changes both between reads. | [application-forwarding](#contract-application-forwarding), [snapshot-history](#contract-snapshot-history) |
| public-detail | Anonymous detail returns public completed metadata and contiguous ordered public operations, excluding command receipts, retry identifiers and internal state. | [http-boundary](#contract-http-boundary), [public-dto](#contract-public-dto), [metadata-invariants](#contract-metadata-invariants), [snapshot-history](#contract-snapshot-history) |

## 宣言した対応関係

矢印は著者が宣言した関係を示す。asserts は記載した事例への直接の assertion、exercises は処理を通る関係を示す。

```mermaid
flowchart TD
    n0["feature: public-record-detail"]
    n1["guarantee: complete-bounded-history"]
    n2["guarantee: hidden-indistinguishable"]
    n3["guarantee: one-view"]
    n4["guarantee: public-detail"]
    n5["contract: application-forwarding"]
    n6["contract: http-boundary"]
    n7["contract: metadata-invariants"]
    n8["contract: public-dto"]
    n9["contract: snapshot-history"]
    n10["test: test-game-persistence-test-contracts-contracts-public-record-detail-metadata-invariants-postgres-enforces-result-pairs-foreign-keys-and-distinct-participants"]
    n11["test: test-game-persistence-test-records-records-public-record-detail-metadata-invariants-current-schema-rejects-missing-metadata-and-inconsistent-finish-time"]
    n12["test: test-game-persistence-test-records-records-public-record-detail-metadata-invariants-finish-time-is-writer-utc-and-retries-preserve-it"]
    n13["test: test-game-persistence-test-records-records-public-record-detail-metadata-invariants-writer-metadata-is-atomic-move-count-excludes-resign-and-retry-keeps-time"]
    n14["test: test-game-persistence-test-records-records-public-record-detail-snapshot-history-corrupt-and-oversized-details-fail-without-partial-history"]
    n15["test: test-game-persistence-test-records-records-public-record-detail-snapshot-history-detail-keeps-one-database-view-when-history-changes-between-reads"]
    n16["test: test-game-persistence-test-records-records-public-record-detail-snapshot-history-public-completed-metadata-only-filters-and-stable-keysets"]
    n17["test: test-game-persistence-test-records-records-public-record-detail-snapshot-history-public-history-accepts-4096-structural-operations-without-replaying-legality"]
    n18["test: test-game-record-application-lib-game-record-application-tests-public-record-detail-metadata-invariants-metadata-construction-requires-distinct-players-and-consistent-finish-counts"]
    n19["test: test-game-record-application-lib-game-record-application-tests-public-record-detail-metadata-invariants-record-scalars-reject-values-outside-storage-bounds"]
    n20["test: test-player-http-lib-player-http-tests-public-record-detail-application-forwarding-anonymous-list-is-metadata-only-and-detail-contains-only-public-ordered-actions"]
    n21["test: test-player-http-lib-player-http-tests-public-record-detail-application-forwarding-bounded-detail-and-storage-errors-are-generic"]
    n22["test: test-player-http-lib-player-http-tests-public-record-detail-http-boundary-query-boundary-rejects-bad-filters-bounds-and-cursors-before-storage"]
    n0 --> n1
    n1 --> n6
    n1 --> n7
    n1 --> n9
    n0 --> n2
    n2 --> n6
    n2 --> n9
    n0 --> n3
    n3 --> n5
    n3 --> n9
    n0 --> n4
    n4 --> n6
    n4 --> n8
    n4 --> n7
    n4 --> n9
    n5 -->|exercises| n20
    n5 -->|exercises| n21
    n6 -->|asserts| n20
    n6 -->|asserts| n21
    n6 -->|asserts| n22
    n7 -->|unreviewed| n10
    n7 -->|unreviewed| n11
    n7 -->|unreviewed| n12
    n7 -->|unreviewed| n13
    n7 -->|asserts| n18
    n7 -->|asserts| n19
    n8 -->|asserts| n20
    n9 -->|asserts| n14
    n9 -->|asserts| n15
    n9 -->|asserts| n16
    n9 -->|asserts| n17
```

## コンポーネント契約と根拠

<a id="contract-application-forwarding"></a>

### application-forwarding

GameRecords::detail passes the typed GameId to its repository and returns the result.

実装位置: [crates/game-record-application/src/lib.rs](../../../crates/game-record-application/src/lib.rs#L338)。

- [test-player-http-lib-player-http-tests-public-record-detail-application-forwarding-anonymous-list-is-metadata-only-and-detail-contains-only-public-ordered-actions](#test-test-player-http-lib-player-http-tests-public-record-detail-application-forwarding-anonymous-list-is-metadata-only-and-detail-contains-only-public-ordered-actions) (exercises): The real router traverses this method into the PostgreSQL reader.
- [test-player-http-lib-player-http-tests-public-record-detail-application-forwarding-bounded-detail-and-storage-errors-are-generic](#test-test-player-http-lib-player-http-tests-public-record-detail-application-forwarding-bounded-detail-and-storage-errors-are-generic) (exercises): Fake repository errors traverse this method before HTTP mapping.

未検証: There is no independent unchanged-forwarding test for arbitrary IDs and results.

<a id="contract-http-boundary"></a>

### http-boundary

The detail handler validates the path ID, uses the shared permit and maps repository failures to generic no-store HTTP responses.

実装位置: [crates/player-http/src/lib.rs](../../../crates/player-http/src/lib.rs#L164)。

- [test-player-http-lib-player-http-tests-public-record-detail-application-forwarding-anonymous-list-is-metadata-only-and-detail-contains-only-public-ordered-actions](#test-test-player-http-lib-player-http-tests-public-record-detail-application-forwarding-anonymous-list-is-metadata-only-and-detail-contains-only-public-ordered-actions) (asserts): Missing and ongoing real database records yield identical 404 status/body, and successful detail has no-store.
- [test-player-http-lib-player-http-tests-public-record-detail-application-forwarding-bounded-detail-and-storage-errors-are-generic](#test-test-player-http-lib-player-http-tests-public-record-detail-application-forwarding-bounded-detail-and-storage-errors-are-generic) (asserts): Fake corrupt, oversized and unavailable failures become 500/422/503 and fixed JSON with no-store.
- [test-player-http-lib-player-http-tests-public-record-detail-http-boundary-query-boundary-rejects-bad-filters-bounds-and-cursors-before-storage](#test-test-player-http-lib-player-http-tests-public-record-detail-http-boundary-query-boundary-rejects-bad-filters-bounds-and-cursors-before-storage) (asserts): An ID containing a decoded space returns 400 before the failing repository can return 500.

未検証: The real-router fixture does not create a hidden record; its hidden behavior is supported by the repository test and shared error mapping. Shared admission and byte limits are mapped in public-record-search.

<a id="contract-metadata-invariants"></a>

### metadata-invariants

Metadata requires distinct participants, a terminal sequence, valid storage scalar bounds and finish-count relations appropriate to moves or resignation.

実装位置: [crates/game-record-application/src/lib.rs](../../../crates/game-record-application/src/lib.rs#L224)。

- [test-game-persistence-test-contracts-contracts-public-record-detail-metadata-invariants-postgres-enforces-result-pairs-foreign-keys-and-distinct-participants](#test-test-game-persistence-test-contracts-contracts-public-record-detail-metadata-invariants-postgres-enforces-result-pairs-foreign-keys-and-distinct-participants) (unreviewed): Directory ownership declares this test belongs to the contract; execution of the implementation and strength of assertions require review.
- [test-game-persistence-test-records-records-public-record-detail-metadata-invariants-current-schema-rejects-missing-metadata-and-inconsistent-finish-time](#test-test-game-persistence-test-records-records-public-record-detail-metadata-invariants-current-schema-rejects-missing-metadata-and-inconsistent-finish-time) (unreviewed): Directory ownership declares this test belongs to the contract; execution of the implementation and strength of assertions require review.
- [test-game-persistence-test-records-records-public-record-detail-metadata-invariants-finish-time-is-writer-utc-and-retries-preserve-it](#test-test-game-persistence-test-records-records-public-record-detail-metadata-invariants-finish-time-is-writer-utc-and-retries-preserve-it) (unreviewed): Directory ownership declares this test belongs to the contract; execution of the implementation and strength of assertions require review.
- [test-game-persistence-test-records-records-public-record-detail-metadata-invariants-writer-metadata-is-atomic-move-count-excludes-resign-and-retry-keeps-time](#test-test-game-persistence-test-records-records-public-record-detail-metadata-invariants-writer-metadata-is-atomic-move-count-excludes-resign-and-retry-keeps-time) (unreviewed): Directory ownership declares this test belongs to the contract; execution of the implementation and strength of assertions require review.
- [test-game-record-application-lib-game-record-application-tests-public-record-detail-metadata-invariants-metadata-construction-requires-distinct-players-and-consistent-finish-counts](#test-test-game-record-application-lib-game-record-application-tests-public-record-detail-metadata-invariants-metadata-construction-requires-distinct-players-and-consistent-finish-counts) (asserts): Direct constructors reject listed duplicate-player, zero-sequence and inconsistent move/resignation count cases.
- [test-game-record-application-lib-game-record-application-tests-public-record-detail-metadata-invariants-record-scalars-reject-values-outside-storage-bounds](#test-test-game-record-application-lib-game-record-application-tests-public-record-detail-metadata-invariants-record-scalars-reject-values-outside-storage-bounds) (asserts): Direct scalar conversion assertions reject values outside PostgreSQL signed-integer storage bounds.

<a id="contract-public-dto"></a>

### public-dto

DetailDto emits metadata and ordered action DTOs without internal request, receipt, check or outbox fields.

実装位置: [crates/player-http/src/lib.rs](../../../crates/player-http/src/lib.rs#L343)。

- [test-player-http-lib-player-http-tests-public-record-detail-application-forwarding-anonymous-list-is-metadata-only-and-detail-contains-only-public-ordered-actions](#test-test-player-http-lib-player-http-tests-public-record-detail-application-forwarding-anonymous-list-is-metadata-only-and-detail-contains-only-public-ordered-actions) (asserts): The exact metadata key set, equality with list metadata and two ordered operations are asserted; serialized detail excludes each listed secret substring.

未検証: This assertion covers one move and one resignation, not every move, drop and terminal-result encoding.

<a id="contract-snapshot-history"></a>

### snapshot-history

The reader uses one repeatable-read transaction and validates a complete contiguous structural history before returning Detail.

実装位置: [crates/game-record-persistence/src/lib.rs](../../../crates/game-record-persistence/src/lib.rs#L309)。

- [test-game-persistence-test-records-records-public-record-detail-snapshot-history-corrupt-and-oversized-details-fail-without-partial-history](#test-test-game-persistence-test-records-records-public-record-detail-snapshot-history-corrupt-and-oversized-details-fail-without-partial-history) (asserts): Eight corruption mutations and excess history are rejected rather than returning partial detail.
- [test-game-persistence-test-records-records-public-record-detail-snapshot-history-detail-keeps-one-database-view-when-history-changes-between-reads](#test-test-game-persistence-test-records-records-public-record-detail-snapshot-history-detail-keeps-one-database-view-when-history-changes-between-reads) (asserts): A second connection changes metadata and history while history reading is blocked; returned detail retains the prior consistent snapshot.
- [test-game-persistence-test-records-records-public-record-detail-snapshot-history-public-completed-metadata-only-filters-and-stable-keysets](#test-test-game-persistence-test-records-records-public-record-detail-snapshot-history-public-completed-metadata-only-filters-and-stable-keysets) (asserts): Repository lookup directly asserts a hidden completed record is not found.
- [test-game-persistence-test-records-records-public-record-detail-snapshot-history-public-history-accepts-4096-structural-operations-without-replaying-legality](#test-test-game-persistence-test-records-records-public-record-detail-snapshot-history-public-history-accepts-4096-structural-operations-without-replaying-legality) (asserts): All 4096 structural operations are accepted, while 4097 are rejected without replaying board legality.

未検証: These are selected corruption mutations rather than an exhaustive matrix. Structural validation deliberately does not establish shogi move legality.

## 実在するテストと実行コマンド

コマンドは個別テストを厳密一致で選択する。保存層テストには TEST_DATABASE_URL と TEST_VALKEY_URL が必要な場合がある。

<a id="test-test-game-persistence-test-contracts-contracts-public-record-detail-metadata-invariants-postgres-enforces-result-pairs-foreign-keys-and-distinct-participants"></a>

### test-game-persistence-test-contracts-contracts-public-record-detail-metadata-invariants-postgres-enforces-result-pairs-foreign-keys-and-distinct-participants

テスト位置: [crates/game-persistence/tests/trace/public-record-detail/metadata-invariants/contracts.rs](../../../crates/game-persistence/tests/trace/public-record-detail/metadata-invariants/contracts.rs#L4)。

範囲: Contract-directory membership only; runtime dependencies and assertions have not been reviewed.

```sh
cargo test --locked -p game-persistence --test contracts contracts::public_record_detail_metadata_invariants::postgres_enforces_result_pairs_foreign_keys_and_distinct_participants -- --exact --format pretty --color never
```

<a id="test-test-game-persistence-test-records-records-public-record-detail-metadata-invariants-current-schema-rejects-missing-metadata-and-inconsistent-finish-time"></a>

### test-game-persistence-test-records-records-public-record-detail-metadata-invariants-current-schema-rejects-missing-metadata-and-inconsistent-finish-time

テスト位置: [crates/game-persistence/tests/trace/public-record-detail/metadata-invariants/records.rs](../../../crates/game-persistence/tests/trace/public-record-detail/metadata-invariants/records.rs#L61)。

範囲: Contract-directory membership only; runtime dependencies and assertions have not been reviewed.

```sh
cargo test --locked -p game-persistence --test records records::public_record_detail_metadata_invariants::current_schema_rejects_missing_metadata_and_inconsistent_finish_time -- --exact --format pretty --color never
```

<a id="test-test-game-persistence-test-records-records-public-record-detail-metadata-invariants-finish-time-is-writer-utc-and-retries-preserve-it"></a>

### test-game-persistence-test-records-records-public-record-detail-metadata-invariants-finish-time-is-writer-utc-and-retries-preserve-it

テスト位置: [crates/game-persistence/tests/trace/public-record-detail/metadata-invariants/records.rs](../../../crates/game-persistence/tests/trace/public-record-detail/metadata-invariants/records.rs#L83)。

範囲: Contract-directory membership only; runtime dependencies and assertions have not been reviewed.

```sh
cargo test --locked -p game-persistence --test records records::public_record_detail_metadata_invariants::finish_time_is_writer_utc_and_retries_preserve_it -- --exact --format pretty --color never
```

<a id="test-test-game-persistence-test-records-records-public-record-detail-metadata-invariants-writer-metadata-is-atomic-move-count-excludes-resign-and-retry-keeps-time"></a>

### test-game-persistence-test-records-records-public-record-detail-metadata-invariants-writer-metadata-is-atomic-move-count-excludes-resign-and-retry-keeps-time

テスト位置: [crates/game-persistence/tests/trace/public-record-detail/metadata-invariants/records.rs](../../../crates/game-persistence/tests/trace/public-record-detail/metadata-invariants/records.rs#L4)。

範囲: Contract-directory membership only; runtime dependencies and assertions have not been reviewed.

```sh
cargo test --locked -p game-persistence --test records records::public_record_detail_metadata_invariants::writer_metadata_is_atomic_move_count_excludes_resign_and_retry_keeps_time -- --exact --format pretty --color never
```

<a id="test-test-game-persistence-test-records-records-public-record-detail-snapshot-history-corrupt-and-oversized-details-fail-without-partial-history"></a>

### test-game-persistence-test-records-records-public-record-detail-snapshot-history-corrupt-and-oversized-details-fail-without-partial-history

テスト位置: [crates/game-persistence/tests/trace/public-record-detail/snapshot-history/tests.rs](../../../crates/game-persistence/tests/trace/public-record-detail/snapshot-history/tests.rs#L199)。

範囲: Real PostgreSQL journal/metadata corruption and oversized-history rejection.

```sh
cargo test --locked -p game-persistence --test records records::public_record_detail_snapshot_history::corrupt_and_oversized_details_fail_without_partial_history -- --exact --format pretty --color never
```

<a id="test-test-game-persistence-test-records-records-public-record-detail-snapshot-history-detail-keeps-one-database-view-when-history-changes-between-reads"></a>

### test-game-persistence-test-records-records-public-record-detail-snapshot-history-detail-keeps-one-database-view-when-history-changes-between-reads

テスト位置: [crates/game-persistence/tests/trace/public-record-detail/snapshot-history/tests.rs](../../../crates/game-persistence/tests/trace/public-record-detail/snapshot-history/tests.rs#L244)。

範囲: Concurrent real PostgreSQL mutation between reader statements tests snapshot consistency.

```sh
cargo test --locked -p game-persistence --test records records::public_record_detail_snapshot_history::detail_keeps_one_database_view_when_history_changes_between_reads -- --exact --format pretty --color never
```

<a id="test-test-game-persistence-test-records-records-public-record-detail-snapshot-history-public-completed-metadata-only-filters-and-stable-keysets"></a>

### test-game-persistence-test-records-records-public-record-detail-snapshot-history-public-completed-metadata-only-filters-and-stable-keysets

テスト位置: [crates/game-persistence/tests/trace/public-record-detail/snapshot-history/tests.rs](../../../crates/game-persistence/tests/trace/public-record-detail/snapshot-history/tests.rs#L11)。

範囲: Real PostgreSQL public-record filtering and hidden-detail lookup.

```sh
cargo test --locked -p game-persistence --test records records::public_record_detail_snapshot_history::public_completed_metadata_only_filters_and_stable_keysets -- --exact --format pretty --color never
```

<a id="test-test-game-persistence-test-records-records-public-record-detail-snapshot-history-public-history-accepts-4096-structural-operations-without-replaying-legality"></a>

### test-game-persistence-test-records-records-public-record-detail-snapshot-history-public-history-accepts-4096-structural-operations-without-replaying-legality

テスト位置: [crates/game-persistence/tests/trace/public-record-detail/snapshot-history/tests.rs](../../../crates/game-persistence/tests/trace/public-record-detail/snapshot-history/tests.rs#L288)。

範囲: Real PostgreSQL structural history boundary; intentionally does not replay legality.

```sh
cargo test --locked -p game-persistence --test records records::public_record_detail_snapshot_history::public_history_accepts_4096_structural_operations_without_replaying_legality -- --exact --format pretty --color never
```

<a id="test-test-game-record-application-lib-game-record-application-tests-public-record-detail-metadata-invariants-metadata-construction-requires-distinct-players-and-consistent-finish-counts"></a>

### test-game-record-application-lib-game-record-application-tests-public-record-detail-metadata-invariants-metadata-construction-requires-distinct-players-and-consistent-finish-counts

テスト位置: [crates/game-record-application/src/trace/public-record-detail/metadata-invariants/tests.rs](../../../crates/game-record-application/src/trace/public-record-detail/metadata-invariants/tests.rs#L44)。

範囲: Pure metadata constructor cases without a database or legal game replay.

```sh
cargo test --locked -p game-record-application --lib tests::public_record_detail_metadata_invariants::metadata_construction_requires_distinct_players_and_consistent_finish_counts -- --exact --format pretty --color never
```

<a id="test-test-game-record-application-lib-game-record-application-tests-public-record-detail-metadata-invariants-record-scalars-reject-values-outside-storage-bounds"></a>

### test-game-record-application-lib-game-record-application-tests-public-record-detail-metadata-invariants-record-scalars-reject-values-outside-storage-bounds

テスト位置: [crates/game-record-application/src/trace/public-record-detail/metadata-invariants/tests.rs](../../../crates/game-record-application/src/trace/public-record-detail/metadata-invariants/tests.rs#L11)。

範囲: Pure scalar storage-boundary cases.

```sh
cargo test --locked -p game-record-application --lib tests::public_record_detail_metadata_invariants::record_scalars_reject_values_outside_storage_bounds -- --exact --format pretty --color never
```

<a id="test-test-player-http-lib-player-http-tests-public-record-detail-application-forwarding-anonymous-list-is-metadata-only-and-detail-contains-only-public-ordered-actions"></a>

### test-player-http-lib-player-http-tests-public-record-detail-application-forwarding-anonymous-list-is-metadata-only-and-detail-contains-only-public-ordered-actions

テスト位置: [crates/player-http/src/trace/public-record-detail/application-forwarding/tests.rs](../../../crates/player-http/src/trace/public-record-detail/application-forwarding/tests.rs#L12)。

範囲: Real Axum router, application service and PostgreSQL reader; asserts public serialization and missing/ongoing behavior.

```sh
cargo test --locked -p player-http --lib tests::public_record_detail_application_forwarding::anonymous_list_is_metadata_only_and_detail_contains_only_public_ordered_actions -- --exact --format pretty --color never
```

<a id="test-test-player-http-lib-player-http-tests-public-record-detail-application-forwarding-bounded-detail-and-storage-errors-are-generic"></a>

### test-player-http-lib-player-http-tests-public-record-detail-application-forwarding-bounded-detail-and-storage-errors-are-generic

テスト位置: [crates/player-http/src/trace/public-record-detail/application-forwarding/tests.rs](../../../crates/player-http/src/trace/public-record-detail/application-forwarding/tests.rs#L153)。

範囲: In-process router with fake repository failures; no database.

```sh
cargo test --locked -p player-http --lib tests::public_record_detail_application_forwarding::bounded_detail_and_storage_errors_are_generic -- --exact --format pretty --color never
```

<a id="test-test-player-http-lib-player-http-tests-public-record-detail-http-boundary-query-boundary-rejects-bad-filters-bounds-and-cursors-before-storage"></a>

### test-player-http-lib-player-http-tests-public-record-detail-http-boundary-query-boundary-rejects-bad-filters-bounds-and-cursors-before-storage

テスト位置: [crates/player-http/src/trace/public-record-detail/http-boundary/tests.rs](../../../crates/player-http/src/trace/public-record-detail/http-boundary/tests.rs#L12)。

範囲: In-process router and failing fake repository; includes malformed detail ID.

```sh
cargo test --locked -p player-http --lib tests::public_record_detail_http_boundary::query_boundary_rejects_bad_filters_bounds_and_cursors_before_storage -- --exact --format pretty --color never
```

## 検査と根拠の限界

check はグラフ、ファイル内の一意なテキスト位置、package と target ごとのコンパイル済みテスト名、生成文書の一致を検査する。
run は check の後に共有された Cargo テストを一度ずつ実行し、厳密一致の 1 件が成功し、失敗と無視が 0 件であることを検査する。
この文書には実行済みの成功状態を保存しない。CI の workspace テスト実行結果は、その実行時点の結果となる。
宣言した関連の意味と網羅性はレビューが必要となる。検出できる関連の削除は、必須の関連数、参照先、孤立ノードの規則に違反するものに限られる。

- No mapped test starts a player-server TCP listener. No guarantee is made that structurally readable history consists of legal shogi play.
- Shared HTTP byte/admission bounds and reader role/timeouts are mapped in public-record-search and startup-schema-isolation.
- Source links and successful test execution establish traceable evidence, not semantic proof or complete coverage of these guarantees.

## ディレクトリ方式の検査範囲

Rust 宣言、通常のモジュールとリテラル include! から物理ファイルの所属、package、target、完全なテスト名と位置を抽出する。ホストのデフォルト Cargo feature 構成で workspace の lib、bin、integration test の全テスト一覧と一致させる。別 target、feature、cfg の組合せは対象外。
src/trace/<feature> と tests/trace/<feature> は新規機能も探索し、未宣言または未到達のファイルを拒否する。契約内の通常テストは unreviewed として追加する。asserts または exercises のレビュー済み根拠がない契約には gap が必要。
全 target の未分類テストは拒否する。trace ディレクトリ内の未対応の item macro と cfg は直接拒否し、外側の未対応構文が生成した実行対象テストはコンパイル済み一覧との差として拒否する。テスト補助とインフラの明示分類および理由は全機能の一覧に示す。
