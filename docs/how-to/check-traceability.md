# 機能・境界契約・テストの対応を更新する

HTML viewer は Rust の宣言と実在する Cargo テストから生成する。
保証やテスト一覧を手書きの文書に複写しない。
保証、契約、検証範囲、根拠、未検証事項の説明は日本語で書く。
ID、列挙値、関数名、パスなどのコード上の識別子は維持する。
[共通の型とマクロ](../../scripts/traceability_model.rs)と[対局操作の機能定義](../../crates/realtime-gateway/src/trace/features/game-play.rs)を参照する。

## 宣言とテストを更新する

1. 変更する責務のコンポーネントを確認する。
   コンポーネントは workspace の Cargo package と対応し、ID と所属は Cargo metadata から収集する。
   package の説明、内部制約の `notes`、外部当事者の `external`、操作一覧の `operations`、ワーカーなどの `entrypoints` は `src/trace/component.rs` に宣言する。
   外部当事者の ID は workspace 全体で一度だけ定義する。
   内部アルゴリズムを収録するためだけに feature や境界契約を作らない。
2. フロントエンド利用者への保証を変える場合は、入口を提供する package の `src/trace/features/<feature>.rs` を更新する。
   同じ機能は workspace 全体で一つだけ定義する。
   `Feature` に保証、処理の流れ、限界を書き、各 `Guarantee.roots` でフロントエンド契約のグローバル ID を参照する。
   根は `frontend` を consumer とする HTTP、gRPC、WebSocket の契約とする。
   ワーカーや起動処理の契約を feature へ無理に接続しない。
3. 直接やり取りする当事者間の仕様を `src/trace/contracts/<contract>.rs` に宣言する。
   通常の API は提供元 package、port はインターフェースを定義する package、外部サービスへの契約は workspace 内の呼出元が仕様を所有する。
   仕様所有者、`consumer`、`provider`、テストの配置先を区別する。
   port の所有者を consumer と同一と仮定しない。
   宣言内のローカル `id` は収集時に `<owner>/<id>` となり、他の宣言やテストからはこのグローバル ID で参照する。
   `source` にインターフェース、`call` に呼出箇所、`implementation` に提供元の実装、`binding` に組立箇所を指定する。
   workspace 内の consumer には `call` が必要となる。
   `Port` には実装と組立の参照も必要となる。
   [repository port の例](../../crates/game-record-application/src/trace/contracts/record-detail-store.rs)を参照する。
4. 上位契約を満たすために提供元が使う下位契約を、`Contract.requires` に理由付きで参照する。
   上位の `provider` と下位の `consumer` を一致させる。
   同じ feature に属することや同じテストで実行することだけを理由に接続しない。
   自己参照、重複、循環、隣接しない接続は検査で拒否する。
   非同期ワーカーは実際のストアや配信先との契約として記録する。
   読み取り API からワーカーへの架空の呼出関係を作らない。
5. テストの目的に合う場所へ配置し、通常の Rust モジュールとして読み込む。
   公開インターフェースを使って境界を検証するテストは integration target の `tests/contracts/` 配下に置く。
   例えば `tests/contracts.rs` から `#[path = "contracts/<contract>/tests.rs"] mod contract_tests;` で接続する。
   ディレクトリ名だけでモジュールは読み込まれない。
   private 要素を使う内部テストは `src/` 配下で実装モジュールから接続する。
   公開 API だけを使う内部の補助的な検査は `tests/internal/` 配下にも置ける。
   一つのテストで使う準備処理はそのテストと同じモジュールに置く。
   共有する準備処理は同じコンポーネントの support モジュールへまとめる。
   `include_str!` などの相対パスは移動後も同じ入力を指すように直す。
   テスト移動のために private fixture を製品 API として公開しない。
   公開関数を使うだけで境界テストに分類せず、検証する契約と assertion を確認する。
6. 境界テストを `trace::boundary_case!(trace::BoundaryCase { evidence: &[...] }; <test item>)` で包む。
   各 `Evidence` に契約 ID、`relationship`、`scope`、`reason`、`observation`、`real`、`fake` を指定する。
   `asserts` は記載した事例を直接検証する関係、`exercises` は処理を通る関係、`unreviewed` は未レビューの関係とする。
   `ProviderBoundary` はテストから提供元を呼ぶ検証を表し、製品の consumer を含む相互作用は `CompleteInteraction` とする。
   対象契約の provider を必ず `real` に含める。
   `CompleteInteraction` では consumer も `real` に含める。
   置き換えた当事者は `fake` に明記し、対象 provider を fake にしたテストを根拠にしない。
   複数契約を検証する場合は同じ `evidence` 配列に追加し、テスト本体を複製しない。
7. 直接の `asserts` がない契約には `gap: Some("未検証事項")` を残す。
   `exercises`、`unreviewed`、内部テストだけではこの条件を満たさない。
   下位契約のテスト成功は上位契約の直接検証にならない。
   `asserts` がある場合も、未検証の条件や障害を `gap`、前提、限界に記録する。
   validator は宣言の整合性を検査するため、実際の呼出経路と assertion の妥当性はレビューと実行で確認する。
8. 内部テストには契約の evidence を付けない。
   `src/` または `tests/internal/` の通常の `#[test]` と `#[tokio::test]` は内部テストとして収集する。
   製品の契約を検証しない基盤検査は `trace::infrastructure!("category", "reason"; <test item>)` で分類する。
   別プロセスから起動する補助テストは `trace::helper!("reason"; <test item>)` で分類する。
   理由はテストごとに宣言し、未分類テストをまとめて除外しない。
9. 宣言をテスト専用の `trace_metadata` モジュールへ `#[path]` で接続する。
   package ごとに lib または bin の一つのテスト target から `#[cfg(test)] mod trace_metadata;` で読み込む。
   共通モデルは `trace_support` として読み込む。
   integration target も共通モデルを読み込むが、component、feature、contract の宣言は再読込しない。
   [宣言の接続例](../../crates/game-record-application/src/trace_metadata.rs)を参照する。
   package、Cargo target、完全なテスト名、ソース位置を別の一覧へ手で登録しない。
10. 操作を追加・削除する場合は `Component.operations` を更新する。
    操作ごとに契約と `Surface` を指定し、`Audience::Frontend`、`Internal`、`Operator` を区別する。
    frontend 操作には feature の根となる契約を指定する。
    内部・運用操作には分類理由を記載し、feature の根へ接続しない。
    OpenAPI 出力と proto の全操作は引き続き照合する。
    WebSocket は route とコマンドの source anchor を宣言する。
    任意の関数を `entrypoints` に登録しても feature の入口条件は満たさない。
11. Cargo example のソース直下に `example!(Example { kind, reason })` を一つ宣言する。
    [OpenAPI 出力の例](../../crates/player-http/examples/player-openapi.rs)と同様に、`ExampleKind::OpenApi` または `Program` と理由を指定する。
    全 example をコンパイルし、`OpenApi` だけを実行して操作一覧を照合する。
    `Program` は実行しない。
    未宣言、重複、条件付きの宣言は拒否する。

## 検査と実テストを実行する

リポジトリのルートで検査ツールのテストと構造検査を実行する。

```sh
python3 scripts/test-traceability.py
python3 scripts/test-check-traceability-migration.py
python3 scripts/test-directory-traceability.py
python3 scripts/test-traceability-viewer.py
python3 scripts/traceability.py check
```

次に実テストを実行する。

```sh
scripts/test-postgres.sh python3 scripts/traceability.py run
scripts/test-postgres.sh cargo test --locked --workspace
```

`check` は宣言、ソース参照、操作の所属、Cargo のコンパイル済みテスト一覧を全対象で照合する。
`check` と `run` は表示用ファイルを読み書きしない。
`run` の既定の `--kind all` は境界・内部・基盤のテストを重複排除して実行する。
helper は親テストから起動し、単独の実行対象には含めない。

分類を絞る場合は次のように指定する。

```sh
scripts/test-postgres.sh python3 scripts/traceability.py run --kind boundary
scripts/test-postgres.sh python3 scripts/traceability.py run --kind internal
scripts/test-postgres.sh python3 scripts/traceability.py run --kind infrastructure
```

`--kind` で実行対象を絞っても、構造と Cargo 一覧の検査は全対象に行う。
テスト本体の成功は `run` または `cargo test` で確認する。
契約グラフは責任の分解を表し、保証の合成や実行時 call graph を証明するものではない。

## ブラウザーで保証とソースを確認する

リポジトリのルートで HTML を生成する。

```sh
python3 scripts/traceability.py viewer
```

`target/traceability-viewer.html` をブラウザーで開く。
サーバーや追加のパッケージは不要で、`file://` でも利用できる。
出力先を変える場合は `python3 scripts/traceability.py viewer --output /tmp/traceability.html` を使う。
`--output` は `viewer` 専用となる。
`--root` を指定した場合、既定の出力先はそのルートの `target/traceability-viewer.html` となる。
入力ソース、追跡済みファイル、ツールのコード、Git の管理ファイルへの上書きは拒否する。

機能またはコンポーネントを検索し、保証・契約・テストや、それらを結ぶ関係を選択する。
機能からは保証の入口と `requires` による下位契約、境界テストをたどる。
コンポーネントからは提供・利用する契約と、所属する内部・基盤・補助テストを確認する。
feature を持たないワーカーもコンポーネントから確認できる。
共有契約は同じ ID を参照する。
詳細には処理の流れ、根拠、検証範囲、実当事者と fake、未検証事項、限界を表示する。
内部テストを契約の evidence として表示しない。
選択項目は URL のハッシュに記録され、再読込とブラウザーの戻る・進むで復元する。
共有する場合は、同じ HTML とハッシュを渡す。
Tab と Enter で項目を選択できる。
ソース画面は **閉じる** または Escape で閉じる。

ソース画面は生成時点に取り込んだ内容と参照行を表示する。
取り込み時の HEAD とファイルの生バイトが一致し、対応する GitHub origin がある場合だけ、そのコミットに固定したリンクを表示する。
変更済み・新規のファイルや Git のない環境でも、取り込んだソースを参照できる。
ファイル群全体の同時点性は保証しない。
ソースを変更したら HTML を再生成する。
HTML には参照されたソースの全文を含むため、共有先を確認する。

`viewer` は構造・操作の所属・Cargo のテスト一覧を検査してから生成する。
テスト本体は実行せず、テストの成功や検証の十分さも示さない。
Rust CI の `traceability-viewer` artifact からも HTML をダウンロードして開ける。

## 検査の対象

分類の対象は、workspace の各 package にあるデフォルト構成の lib、bin、integration test target。
新しい package や target も列挙し、未分類のコンパイル済みテストを拒否する。
テストがない target も検査対象として記録する。
`test = false` の target は理由付きで記録し、`required-features` を持つ target は構成を推測せず拒否する。
基盤・補助テスト、除外 target、example の分類と理由は収集・検査する。
基盤・補助テストは viewer のコンポーネントからも確認できる。
除外 target と example の理由は Rust 宣言と Cargo.toml で確認する。
ディレクトリ規約、ファイルの到達、同一テストの重複、宣言とコンパイル結果の不一致も検査する。

[収集ツール](../../tools/traceability/Cargo.toml) は独立した Rust workspace で、syn による構文解析を使う。
本体の依存関係には追加しない。
検査はホスト環境のデフォルト Cargo 構成で行い、別 target・feature・cfg の組合せを網羅するものではない。
例や doctest、Python・shell の検査は、この Cargo テスト分類の対象外とし、それぞれの CI 検査を継続する。
保証の意味、テストとの関連の妥当性、検証の十分さは引き続きレビューする。

Python 3.11 以上と Rust のビルド環境を使う。
実テストを実行する最後の手順には Docker Compose も必要となる。
既存の検証サービスを使う場合の設定は[ワークスペースの検証手順](../../README.md#ワークスペースを検証する)を参照する。
