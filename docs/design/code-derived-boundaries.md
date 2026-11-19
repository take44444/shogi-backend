# コードから生成するコンポーネント境界とテスト対象

コンポーネント間の呼出関係は、コンパイルした Rust の定義と呼出から生成する。
各テストの意図は、テストに付けた `targets` 属性で明示する。
関係を別ファイルに登録する方式は使わない。

## 生成する関係

```text
game-application ── calls ──> shogi-domain::Game::play
                                      ↑ targets
                         rejected_move_preserves_position

game-grpc ── calls ──> game-application::MatchmakingRepository::ready
```

コンポーネントは Cargo workspace package とする。
呼出先には公開関数、公開メソッド、関連関数、constructor、trait メソッドを含める。
`dyn Trait` とジェネリクスの呼出は、宣言された trait メソッドに対応付ける。
呼出元と trait の定義元が同じ package でも、インターフェースへの依存として残す。
注入された具体型の追跡と、実装関係のグラフは生成しない。

HTTP、gRPC、メッセージの送受信を対応付ける通信グラフは対象外とする。
HTTP adapter から application への通常の Rust 呼出は対象に含める。

## テストの対象を属性で宣言する

```rust
#[traceability::targets(shogi_domain::Game::play)]
#[test]
fn rejected_move_preserves_position() {
    // 対象の振る舞いを検証する。
}
```

属性はテストの関数本体を変更せず、コンパイル用の非実行 marker を追加する。
marker は指定された Rust の項目を参照し、コンパイラがその参照先を解決する。
抽出器は解決した定義 ID を使ってテストと API を結ぶ。
文字列の契約 ID や、呼出の到達可能性からテストの対象を作らない。

trait のインターフェースを対象にするテストは `<dyn Trait>::method` を指定できる。
具体的な実装を対象にするテストは、その実装型のメソッドを指定する。
この二つの定義 ID を区別する。
製品コードの trait 呼出を宣言に対応付ける処理を、テスト対象の正規化に流用しない。
具体的な実装をテストする事実から、trait 境界のテストとは判定しない。

対象属性を持たないテストも一覧に含める。
通信を検証するテストに便宜的な constructor を割り当てない。
対象の指定とテストの成功は別の情報であり、いずれも契約の十分な検証を自動証明しない。

## コンパイラが解決した情報を収集する

解析用の Rust を日付で固定し、`rustc_driver` を Cargo の workspace wrapper として実行する。
製品コードの通常ビルドには nightly 機能を追加しない。
Cargo が渡す features、target、cfg、build script と proc macro の結果を引き継ぐ。

コンパイラの型情報と構造化された中間表現から呼出を収集する。
マクロ、別名、再公開による表記差は、コンパイラによる名前解決後の定義として扱う。
人間向けの MIR テキストや、特定の構文を探す文字列パターンは製品の抽出に使わない。

通常の関数呼出と、関数を引数として渡す参照を別に保存する。
例えば `.map(UserId::new)` で関数項目を渡しただけでは、その箇所からの直接呼出とは表示しない。
関数ポインタなど、呼出先の定義を特定できない呼出は未解決として残す。
コンパイラによる暗黙の処理も集計し、通常の呼出と区別する。

数値のセッション内 `DefId` をそのまま永続 ID にしない。
Cargo の所属、ソースの出自、コンパイラが解決した定義から ID を作る。
同じ定義の通常 lib build、lib-test build、依存として参照する build の ID を照合する。
同名の別メソッドを名前だけで結合しない。
マクロが同じソース位置から複数の実装を生成する場合は、実装する trait と Self 型の意味情報を含めて区別する。
型と trait の generic 引数も含め、通常の lib build と lib-test build で同じ API の ID を保つ。
異なる定義が同じ ID になった場合は、後勝ちで上書きせず解析の失敗として報告する。
生成ソースの出自には一時的な build hash を含めず、その内容も生成物に取り込む。

## 一つの生成モデルに集約する

生成モデルは次の情報を持つ。

| 情報 | 内容 |
| --- | --- |
| components | Cargo workspace package と相対パス |
| apis | コンパイラが解決した API の ID、所属、定義位置 |
| calls | 呼出元、呼出先、呼出位置、dispatch の種類 |
| references | 関数項目の参照と受け渡し |
| tests | Cargo target、テスト名、定義位置、属性の対象 |
| units | 実際に解析したコンパイル単位 |
| coverage | 収集した body、呼出形式、外部呼出の集計 |
| unresolved | 呼出先を解決できない箇所と理由 |
| configuration | コンパイラと解析設定 |

各コンパイラプロセスは別々の shard を書く。
最後の merger が ID と参照の整合性を検査して、一つの report を生成する。
複数プロセスが同じ JSON を追記しない。
CLI と viewer は同じ report を読む。

新しい収集でも全 workspace unit を解析するよう、workspace wrapper の識別を実行ごとに変える。
依存ライブラリのビルドキャッシュは利用する。
通常の lib と bin をビルドしてから、テストをコンパイルする。
テスト準備用の呼出を製品の依存関係として表示しないよう、それぞれの呼出にコンパイル元を保持する。
コンパイラが出した unit と Cargo の実際の artifact を照合し、キャッシュにより shard が欠けた場合は失敗にする。
コンパイル済みテスト実行ファイルの `--list` と、コンパイラから抽出したテストを完全一致で照合する。
存在しないソース位置や仮のテスト情報で欠落を埋めない。

各 unit に実際に有効だった feature と、通常ビルド・テストビルドの用途を記録する。
dev-dependency により feature が有効になった通常 lib も、製品コードの呼出には混ぜない。
キャッシュを共有した unit は両方の用途を保持する。

## 抽出範囲と未解決を表示する

結果が表すのは、記録されたビルド設定でコンパイルしたコードである。
無効な feature、異なる target、`cfg` で除外したコードまで収集したとは扱わない。
対象外になった Cargo target は別に記録する。

関数ポインタ、外部 generic の内部、ライブラリに渡した callback の実行まで、任意の実行経路を復元するとは主張しない。
未解決の件数がゼロというだけで全実行経路の網羅性を証明したとは扱わない。
生成する呼出関係と、実際にその経路を通ったという実行証拠を区別する。

## 既存方式を削除する

旧共通モデル、コンポーネントごとの関係宣言、契約 ID を指定するテスト wrapper を削除する。
旧方式の parser、validator、fixture、操作手順も置き換える。
旧宣言を新しい解析の fallback や、生成結果を補う登録表として残さない。
テストの実装と実行可能なテスト一覧は保持する。

境界の抽出とテスト対象の照合は compiler fixture で検証する。
別名、マクロ、async、dyn、ジェネリクス、具体型の trait 実装を含める。
実装対象とインターフェース対象が混同されない反例を含める。
warm cache で二回収集しても結果が変わらないことを検証する。

Model the Domain に従い、呼出、参照、テスト対象を別の情報にした。
Sequence Work into Verifiable Units に従い、属性、抽出器、テスト移行、CLI と viewer を検証可能な単位で切り替える。

関連する compiler API は [rustc_driver](https://rustc-dev-guide.rust-lang.org/rustc-driver/intro.html) と [Rust の型情報](https://rustc-dev-guide.rust-lang.org/ty.html)を参照。
