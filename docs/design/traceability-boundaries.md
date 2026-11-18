# フロントエンド機能とコンポーネント境界を分ける traceability 設計案

フロントエンド利用者への保証から、直接やり取りするコンポーネント間の契約と、その境界を検証するテストをたどれるようにする。
内部テストは所属コンポーネントの中で管理する。
本書は実装前の提案であり、既存の宣言、テスト配置、ツールの動作はまだ変更しない。

## 現状と変更理由

[共通モデル](../../scripts/traceability_model.rs)の `Contract` は説明、実装ソース、gap だけを持つ。
契約の当事者や上下関係を持たない。
[Rust collector](../../tools/traceability/src/main.rs)の `owner()` は `src/trace/<feature>/<contract>` と `tests/trace/<feature>/<contract>` から所属を決める。
通常のテストも契約に所属させる必要がある。
[Python validator](../../scripts/traceability.py) は feature ごとの平坦なグラフを検査し、[viewer](../../scripts/traceability_viewer/viewer.js) は保証、契約、テストの固定三列を表示する。

この構造では、内部テストの収録にも契約が必要になり、内部責務の収録にも feature が必要になる。
実際に [rating-application](../../crates/game-application/src/trace/rating-application/feature.rs) と [startup-schema-isolation](../../crates/game-persistence/src/trace/startup-schema-isolation/feature.rs) はフロントエンド向け操作を持たない。

Git の `d630bcb` は Rust 宣言による収集、`426d6e7` は feature と契約によるテスト配置、`396f786` は生成 Markdown の廃止を導入している。
これらと[現行手順](../how-to/check-traceability.md)から、ソースを正本にすること、通常の Rust モジュールでテストを接続すること、実際の Cargo テスト一覧と照合することを維持する。
平坦な契約階層や内部責務の feature 化が必要だったという判断理由は、確認した履歴には見つからなかった。

## 利用者と保守者から見た構造

利用者は feature 一覧で「着手・投了」「公開棋譜の検索」「公開棋譜の詳細」「レートの取得」を探す。
保証を開くと、最初にフロントエンドとサーバーの契約が現れる。
下位へ展開すると、そのサーバーが保証を満たすために利用する、隣接コンポーネントとの契約が現れる。

保守者はコンポーネント一覧から、提供する契約、利用する契約、内部テスト、基盤検査を確認する。
feature から到達しないワーカーや起動処理も、この一覧から参照できる。
契約とテストは workspace 全体で一つの実体を持ち、複数 feature から参照しても複製しない。

```text
feature
  保証
    フロントエンド → A の契約
      A → B の契約
        B → C の契約
        B → D の契約

各契約 ← その境界を検証するテスト
各コンポーネント ← 所属する内部テスト・基盤検査
```

矢印は契約の呼出元と提供元を表す。
字下げは「上位の契約を満たすために使う下位契約」を表す。
コンポーネントの所有関係やテストの所属とは別の関係である。

## コンポーネントの単位

初回移行では、workspace の各 Cargo package を一つのコンポーネントとする。
`apps` の実行ファイルも同様に扱う。
フロントエンド、PostgreSQL、Valkey、外部認証プロバイダーなど、実際に境界を持つ外部の当事者は外部コンポーネントとして宣言する。
外部コンポーネントの宣言も正本を一か所に置く。

private module は独立コンポーネントにしない。
`shogi-domain` の `adapter::Rules` はその内部である。
現在の `shogi-rules` は保証 ID であり、同名の crate は存在しない。
そのため、アダプターの内部テストに契約を割り当てるためだけのコンポーネントは作らない。
将来、独立したインターフェースと責任を持つ単位へ分離する場合に粒度を見直す。

Component ID と package の対応は Cargo metadata から導出する。
内部モジュールの一覧、実装ファイルの一覧、全テスト名を別台帳へ手で転記しない。

## データモデル

feature ごとの JSON グラフを、workspace 全体の正規化された bundle に置き換える。
以下は型の概念図であり、まだコンパイルする実装ではない。

```text
Bundle
  components     ComponentId → Component
  features       FeatureId → Feature
  contracts      ContractId → Contract
  tests          CargoTestIdentity → Test
  inventories    Cargo target・operation・example・除外理由

Component
  id, package または external, source, description, internal_notes

Feature
  id, title, source, guarantees[], limitations[]

Guarantee
  id, statement, roots: ContractId[]

Contract
  id, owner, consumer, provider
  interface: RustApi | Port | Http | Grpc | WebSocket | External
  statement, assumptions[], limitations[], gap
  interface_source, call_source, implementation_source, binding_source
  requires: { contract: ContractId, reason }[]

TestKind
  Boundary { evidence: Evidence[] }
  Internal { component: ComponentId }
  Infrastructure { component: ComponentId, category, reason }
  Helper { component: ComponentId, reason }

Evidence
  contract: ContractId, relationship, scope, reason
  observation: ProviderBoundary | CompleteInteraction
  participants: 実際に使うコンポーネントと real/fake の区別
```

`ContractId` は宣言所有者とローカル名の組で識別する。
例えば `shogi-domain/application-play`、`game-record-application/record-detail-store` とする。
表記をディレクトリ名へ分解して所属を推測せず、宣言と参照を照合する。
テストの ID は従来同様、package、Cargo target、完全なテスト名で識別する。

契約の仕様は、通常の API なら提供元 package、依存逆転の port なら trait を定義する package が所有する。
外部提供元への契約は、workspace 内の呼出元が所有する。
所有者、呼出元、実行時の提供元、テストの配置先を混同しない。

例えば `RecordRepository` は `game-record-application` が定義する。
実行時の提供元は `game-record-persistence` の `PostgresGameRecords` である。
契約には trait、呼出箇所、`impl RecordRepository`、実際に注入する組立箇所を記録する。
Cargo では persistence が application に依存するが、実行時の契約は application → persistence となる。
Cargo の依存方向だけから呼出方向を決めない。

全契約で両当事者を必須にする。
workspace 内の当事者には該当する source anchor を要求する。
外部当事者では、ローカルの通信実装、依存定義、設定などの参照を使い、存在しない外部ソースを要求しない。
interface、call、implementation、binding の必須項目は interface 種別ごとに検査する。

## 入れ子の契約

上位契約 `A → B` の `requires` には、B が利用する `B → C` の契約を記録する。
各参照には、下位のどの責務が上位のどの保証に必要かを日本語で記載する。
同一の上位契約は複数の下位契約を利用でき、下位契約は複数の上位契約から共有できる。
正本は有向非巡回グラフとし、viewer で入れ子表示する。

`parent.provider == child.consumer` を検査する。
未知参照、自己参照、重複参照、循環、隣接しない接続を拒否する。
consumer と provider が同じコンポーネントの宣言も拒否する。
契約が同じ feature に関係することや、テストで同時に実行されることは、`requires` を結ぶ理由にならない。

下位契約のテスト成功を、上位契約の直接検証として繰り上げない。
グラフは責任の分解を表すもので、前提条件の充足や動作の合成を証明するものではない。
下位が `asserts` を持っていても、上位に直接の根拠がなければ上位の `gap` を残す。

非同期処理は、実際のキュー、永続ストア、ブローカーを介する相互作用として表す。
読み取り API からワーカーへ架空の呼出契約を作らない。
通常の呼出連鎖として分解できない協調関係は、この移行で新しい証明モデルを導入せず、独立した契約と説明・gap として残す。
全契約に feature からの到達性を要求しない。

## 実際の経路への適用

着手・投了は次の経路に分ける。
根拠となる実装は [gateway](../../crates/realtime-gateway/src/game.rs)、[gRPC](../../crates/game-grpc/src/service.rs)、[actor](../../crates/game-application/src/actor.rs)、[domain](../../crates/shogi-domain/src/lib.rs) にある。

```text
game-play の保証
  frontend → realtime-gateway の WebSocket コマンド契約
    realtime-gateway → game-grpc の MakeMove / Resign 契約
      game-grpc → game-application のコマンド受理契約
        game-application → shogi-domain の着手・投了契約
        game-application → game-persistence の受理・永続化契約
```

各行は一つ以上の具体的な契約を略記している。
feature の根をドメイン関数や内部 gRPC に直接接続しない。
private な `Rules` のアルゴリズム検査は `shogi-domain` の内部テストになる。
公開 `Game` API を利用する application → domain の境界テストとは分ける。

公開棋譜詳細は次の経路になる。

```text
public-record-detail の保証
  frontend → player-http の GET /game-records/{id} 契約
    player-http → game-record-application の詳細取得契約
      game-record-application → game-record-persistence の repository 契約
        game-record-persistence → PostgreSQL の読み取り契約
```

[現在の application-forwarding テスト](../../crates/player-http/src/trace/public-record-detail/application-forwarding/tests.rs)は、実ルーターと実 DB を通る複数境界のテストである。
その存在だけで全境界を `asserts` に変更しない。
HTTP 応答を検査する関係、repository を通過するだけの関係など、現在の説明を読み直して対応付ける。
`metadata-invariants` のコンストラクター検査は application 内部へ戻す。

`worker-reconciliation` は、実 `RatingWorker` と実 PostgreSQL repository のコミット前後に失敗を注入する[既存テスト](../../crates/game-persistence/tests/trace/rating-application/worker-reconciliation/tests.rs)を持つ。
これは game-application → game-persistence の境界根拠として残す。
feature 名 `rating-application` を残すための架空のフロントエンド操作は作らない。
公開レート取得の保証を「即時にジョブ処理が完了する」へ強めない。

## テスト種別と配置

境界テストは、特定した当事者の製品インターフェースを呼び、契約で観測可能とした結果を検査する。
公開関数を呼ぶだけでは境界テストとしない。
契約 ID、当事者、scope、reason の宣言が必要である。

| 分類 | 所属と宣言 | 契約へのリンク |
| --- | --- | --- |
| Boundary | `tests/contracts/` の integration test。`boundary_case!` で対象を明示する | 一つ以上必要 |
| Internal | 実装モジュールに付属する `src/<module>/tests.rs` など。package から所属を導出する | 禁止 |
| Infrastructure | コンポーネント内の依存方針・環境などの検査。カテゴリと理由を明示する | 禁止 |
| Helper | 別テストが起動する補助ケース。理由を明示する | 禁止 |

内部だけを検査する integration test は `tests/internal/` としてもよい。
`tests/` の未分類テストは拒否し、`src` 内の通常の単体テストは Internal として収集する。
境界ディレクトリにあるだけのテストに暗黙の証拠リンクは付けない。
未レビューの境界テストも `Boundary` と `unreviewed` の明示宣言を要求する。

`asserts`、`exercises`、`unreviewed` を維持する。
契約に `asserts` がない場合は `gap` を必須にする。
現在は `exercises` でも gap を省けるため、これは意図した厳格化である。
`asserts` があっても未検証範囲は gap に記載できる。

fake は、呼出元が提供する上位契約を検査するために利用できる。
例えば fake repository を使って実 HTTP ルーターの拒否応答を検査できる。
一方、fake repository は具体的な PostgreSQL 提供元の契約を検証した根拠にはならない。
participants にその違いを明示するが、宣言が実態と一致することや assertion の意味は引き続きレビューが必要になる。

`ProviderBoundary` では、テスト harness が呼出元の役割を担い、実提供元の製品インターフェースを直接検査する。
例えば domain の契約検査のために application 全体を起動する必要はない。
呼出元の前提条件を与えた場合に、提供元が保証を満たす範囲の根拠になる。
`CompleteInteraction` は実呼出元と実提供元の協調を検査する。
前者の結果だけで、本番の呼出元が前提を守ることや、組立・通信が正しいことを検証済みにしない。

evidence ごとに、対象契約の provider が real と宣言されていることを要求する。
`ProviderBoundary` の consumer は明示した harness で代行できる。
`CompleteInteraction` の consumer も real を要求する。
対象 provider を fake に置き換えた宣言で `asserts` や `exercises` を結ぶことを validator が拒否する。
これは宣言内の矛盾の検査であり、実際にその境界を呼んでいるか、何を assertion しているかは実コードと実行で確認する。

境界テストは integration target に置き、製品 crate の private 要素を参照できない構造にする。
private fixture が必要な既存テストは内部に残す。
`Game::from_sfen` は private な `cfg(test)` 関数なので、それを使うテストをそのまま境界へ移さない。
テスト移動のために SFEN インポートなどの製品 API を公開しない。
公開 API で不足する境界根拠は gap とし、必要な代表ケースを別途追加する。

## ディレクトリと Rust モジュール

以下は移行後の配置例であり、新設予定の名前を含む。
本体コードの crate 分割や業務ロジックは変更せず、宣言とテストの責任を配置へ反映する。

```text
crates/realtime-gateway/
  src/trace_metadata.rs
  src/trace/component.rs
  src/trace/features/game-play.rs
  src/trace/contracts/frontend-game-command.rs
  src/game/tests.rs
  tests/contracts.rs
  tests/contracts/frontend-game-command/tests.rs
  tests/support/...

crates/game-record-application/
  src/trace_metadata.rs
  src/trace/contracts/http-record-detail.rs
  src/trace/contracts/record-detail-store.rs
  src/metadata/tests.rs

crates/game-record-persistence/
  src/trace/contracts/postgres-record-read.rs
  src/detail/tests.rs
  tests/contracts.rs
  tests/contracts/record-detail-store/tests.rs
  tests/contracts/postgres-record-read/tests.rs

crates/shogi-domain/
  src/trace/contracts/application-play.rs
  src/trace/contracts/application-resign.rs
  src/tests.rs
  src/adapter/tests.rs
  src/notation/tests.rs
  tests/contracts.rs
  tests/contracts/application-play/tests.rs

crates/game-persistence/
  tests/contracts.rs
  tests/contracts/rating-worker-reconciliation/tests.rs
```

`metadata.rs` や `detail.rs` が現在一つの `lib.rs` 内にある場合、この例のためだけに本体を分割しない。
当面は `src/tests/<subject>.rs` をその実装モジュールから読み込む配置でもよい。
feature 宣言は、フロントエンドとの境界を提供する package に置く。
複数の入口を持つ feature も一か所で定義し、保証ごとに該当するフロントエンド契約を参照する。

契約宣言は API・port の定義元の `src/trace/contracts/` に一つだけ置く。
package の説明、内部制約の参照、担当する外部当事者の宣言は `src/trace/component.rs` に置く。
`#[cfg(test)] mod trace_metadata` から通常の Rust モジュールとして読み込み、lib または bin のテスト target でコンパイルする。
integration target は共通の型とマクロを読み込むが、契約宣言を再読込しない。
別 package が所有する契約もグローバル ID で参照できる。

`tests/contracts.rs` を harness とし、配下の境界テストを通常の子モジュールで接続する。
ハイフンを含むディレクトリ名や `tests.rs` は自動探索されないため、次のように明示する。

```rust
#[path = "../../../scripts/traceability_model.rs"]
mod trace_support;

#[path = "contracts/frontend-game-command/tests.rs"]
mod frontend_game_command;
```

内部テストは対象実装の `#[cfg(test)] mod tests` から接続する。
一つのテストだけの準備処理はそのテスト内、複数テストで共有する準備処理は同じコンポーネントの `tests/support/` に置く。
既存 `pg-test-support` などの共有 fixture は利用を継続する。
fixture の都合だけで、契約の所有者を変更しない。

## feature の再分類と操作一覧

| 現行 feature | 移行先 |
| --- | --- |
| game-play | WebSocket の着手・投了を入口にする feature |
| game-command-recovery | 利用者の再試行・結果確認に関する保証を対局操作へ統合し、残りはコンポーネントの契約・内部テストへ分ける |
| game-snapshots-access | ブラウザーの対局表示・閲覧制御として realtime-game-feed と保証を整理し、内部 RPC は契約へ移す |
| matchmaking-tickets、matchmaking-pairing | フロントエンドの参加・準備確認・対局成立として保証を整理する |
| public-record-search、public-record-detail、user-ratings | 公開 HTTP を入口とする feature として維持する |
| account-google-login、account-profile、account-session | フロントエンド向け feature として維持する |
| realtime-game-feed、realtime-session-commands-limits | 対局表示・操作・接続に関する利用者の保証として重複を整理する |
| rating-application、durable-event-dispatch | ワーカー・ストア・配信先の契約、各コンポーネントの内部テストへ移す |
| startup-schema-isolation | 実際の起動・DB 境界の契約と、コンポーネント内部・基盤の検査へ分ける |
| public-api-documents | フロントエンドが取得する公開仕様の保証を残し、オフライン生成手順は基盤・example 分類へ移す |

DB スキーマ互換性と、フロントエンドに提供する JSON・OpenAPI のスキーマ互換性を区別する。
後者は利用者の保証になり得る。
DB 分離のような構成全体の制約には、無理に二者間契約を割り当てない。
既存の [依存境界検査](../../scripts/check-boundaries.py)、[公開インターフェース検査](../../scripts/check-contract-interfaces.py)、ADR、deployment 検査を維持し、コンポーネント側からその制約・検査ソースを参照する。

操作の一覧は feature から分離する。
全 OpenAPI・proto 操作は引き続き列挙して照合し、frontend 操作か internal 操作かを理由付きで分類する。
frontend 操作には対応するフロントエンド契約と feature を要求する。
内部 gRPC には実際の内部契約を要求するが、feature の直接所有は要求しない。
WebSocket も独立した interface 種別として、upgrade とコマンド・フレームの宣言を検査する。
WebSocket の操作集合は OpenAPI から導出できないため、wire 宣言・route の source anchor と collector の対応検査を別に設ける。
任意の関数を `entrypoint` に登録するだけで feature の入口要件を満たす仕組みは廃止する。
ワーカーの CLI・tick はコンポーネントの入口として残す。

## ツールの責任

| 場所 | 変更内容 |
| --- | --- |
| `scripts/traceability_model.rs` | Component、当事者付き Contract、requires、排他的な TestKind、境界テスト宣言を定義する |
| `tools/traceability/src/main.rs` | feature パスによる主契約の推測を廃止し、package 所属と明示宣言から bundle を収集する |
| `scripts/traceability.py` | グローバル参照、所有者、隣接性、循環、テスト分類、操作一覧、Cargo 一覧を検査する |
| `scripts/traceability_viewer.py` | feature 別グラフでなく bundle 全体と参照ソースを取り込む |
| `scripts/traceability_viewer/viewer.js` | feature 起点の可変深度表示、component 起点の一覧、共有契約の逆参照を表示する |

処理の境界は `collect(root) → Bundle`、`validate(bundle, inventory) → CheckedBundle`、`select_tests(checked, selection) → TestPlan`、`capture(checked) → Snapshot` とする。
CLI と viewer が別々の所属判定や到達性検査を持たない構造にする。
Rust モデルの型追加と collector の literal parser の許可フィールド更新は同じ変更で行う。

`check` は対象全体の構造とコンパイル済み一覧を検査し、実テストの成功は表さない。
feature から到達しない内部契約も、既知の当事者、実在する入口、直接の根拠または gap を要求する。
source needle、重複 ID、到達しない宣言・テストソース、未知の分類、除外理由などの既存検査を維持する。
直接やり取りしているという宣言は source anchor と依存・組立情報で照合するが、syn と Cargo metadata だけで完全な実行時 call graph が証明できるとは扱わない。

`run` は選択された実テストの ID を重複排除して実行する。
既定では Boundary、Internal、Infrastructure を対象にし、Helper は親テストから起動する。
現行 `run` は契約にリンクしたテストだけを実行するため、これは意図した対象拡大である。
`--kind boundary` と `--kind internal` による絞り込みを追加する。
Python・shell・compile-fail 検査は既存のコマンド・CI で継続し、この移行で Cargo テストに偽装しない。

viewer は機能、コンポーネント、契約、全テスト分類の入口を持つ。
feature 画面で内部テストを契約の根拠として展開しない。
component 画面では内部テストも表示する。
共有契約は同じ ID を参照し、参照元一覧と循環しない展開を提供する。
表示順を保証の数や固定階層に依存させない。
ソース全文の取り込み、安全な HTML 出力、キー操作、URL 選択、条件を満たした commit 固定リンクを維持する。
viewer の生成結果をテスト実行結果として表示しない。

## 比較した構造と判断

feature 内の契約ツリーを拡張する案は、入口の変更は小さいが、共有契約の複製と feature に属さない内部責務の扱いが残るため採用しない。
component、provided interface、required interface、binding をすべて独立した台帳にする案は、同じ境界の更新箇所が増えるため採用しない。

component が契約を所有し、参照からグローバルな契約グラフを作る案を採用する。
実行時の当事者と仕様所有者を分け、port の定義元と adapter 実装の違いを保持する。
interface は当面 Contract の種別と source に集約し、共通化が実際に必要になるまで別の登録対象にしない。
契約仕様と境界テストの配置を分けることで、同じ契約を複数の target から検証しても仕様を重複定義しない。

二つの独立した設計案は、component 所有、グローバルな参照、入れ子の表示という構造に収束した。
package 単位と仕様・テストの別配置を明確にした案を基礎とし、もう一案から port 所有者と実提供元の区別、および非同期処理を無理に feature へ接続しない判断を取り入れた。
独立した Interface・Interaction 台帳は初回設計から除いた。
別モデルによる比較でも同じ基礎案が選ばれ、Rust の `#[path]` 接続例と evidence ごとの実提供元の適格条件を追記した。

Model the Domain の原則を、Feature、Contract、TestKind の役割を分ける判断に適用した。
Redesign from First Principles の原則を、現行 feature グラフへ継ぎ足さず bundle の構造を変える判断に適用した。
Laziness Protocol の原則を、package の粒度を維持し、独立 interface 台帳や不要な本体分割を追加しない判断に適用した。

## 移行と受入条件

実装開始前に旧 collector の収集結果、コンパイル済みテストの ID とソース本文、操作一覧を保存する。
collector の既存 `--scan` は Rust テストの移動前後の照合に利用できる。
完全名や target が変わるテストは、旧 ID と新 ID の対応を保存して、意図した移動と欠落を区別する。
単なる件数一致を受入条件にしない。

実装は次の単位で進める。

1. 新モデル、collector、validator の fixture を揃える。共有下位契約、非隣接参照、循環、依存逆転、Internal からの evidence 禁止、内部だけの契約を検査する。
2. `public-record-detail` を具体例に、実 HTTP から repository までの契約とテストを移行する。private setup、fake と実 adapter、`exercises` と `asserts` を確認する。
3. 対局・マッチング・認証・レーティング・配信・起動検査へ同じ規約を適用する。保証文を単にコピーせず、既存の意味と gap を照合する。
4. 新 bundle から viewer と実行対象を生成する。可変深度、共有契約、component の内部テスト、feature のないワーカーを実際の HTML で確認する。
5. 全対象の一覧照合と実テストを行い、旧 feature 所属の collector 経路、旧 `Case.also` の feature 指定、旧ディレクトリ規約を削除する。手順書を更新する。

段階移行が必要な間も、正本は各対象につき一つとする。
旧新を一時併用する場合は、対象と撤去期限を限定した collector 内の変換だけに留める。
最終状態で二つの編集可能な定義や二つのテスト分類規約を残さない。

実装時には次の既存検査に新ケースを追加して実行する。

```sh
python3 scripts/test-traceability.py
python3 scripts/test-directory-traceability.py
python3 scripts/test-traceability-viewer.py
python3 scripts/test-check-boundaries.py
python3 scripts/check-boundaries.py
python3 scripts/check-contract-interfaces.py
python3 scripts/traceability.py check
scripts/test-postgres.sh python3 scripts/traceability.py run
scripts/test-postgres.sh cargo test --locked --workspace
python3 scripts/traceability.py viewer --output /tmp/traceability.html
```

各 feature に実在する frontend 契約があること、各 requires が隣接すること、各テストが一種類に分類されることを受入条件とする。
旧テストの移動先と本文の維持、全操作の分類、共有テストの一度だけの実行も確認する。
根拠のない保証に gap が残ることを確認し、移動だけで検証済み範囲を増やさない。
既定の host・Cargo feature 構成以外を網羅しないという現在の制限は維持する。

この設計段階ではコードやテストを移動せず、上記の実装後検査もまだ実行しない。
次の実装単位は、新しい bundle とテスト分類を collector の fixture で表し、`public-record-detail` の一経路を新構造へ通すことである。
