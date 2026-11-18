use crate::trace_support as trace;
trace::component!(trace::Component {
    description: "game-record-persistence コンポーネント",
    notes: &[
        "public-record-detail/snapshot-history。読み取り処理は一つの反復可能読み取りトランザクションを使い、連続した履歴全体の構造を検証してから Detail を返す。 未検証範囲。破損の変更事例を選んで確認しており、すべての組み合わせは網羅しない。構造の検証では、意図的に将棋の着手の合法性は確認しない。",
        "public-record-search/public-keysets。PostgreSQL の読み取り処理は、公開された終了済み対局のメタデータを対象に指定された全フィルターを適用し、直前の終了時刻とバイナリー形式の ID の組より後の結果だけを取得する。",
        "startup-schema-isolation/reader-boundary。PostgresGameRecords::connect は既存スキーマを初期化せず検証し、接続プールを四つに制限する。トランザクションを読み取り専用に設定し、接続取得・文実行・ロックのタイムアウトを二秒にする。 未検証範囲。lock_timeout の設定は確認しているが、実際にロック待ちでタイムアウトする別の事例は対応付けられていない。",
        "user-ratings/postgres-snapshot。PostgreSQL 読み取り側は指定ユーザーの確定済み rating_states を読み、存在しないプールを Unrated とし、保存値を RatingState で検証する。 未検証範囲。読み取り側での破損値拒否、別ユーザーの値が混ざらないこと、未知ユーザーの両プールの初期値を個別に確認する assertion はない。"
    ],
    external: &[],
    operations: &[],
    entrypoints: &[trace::EntryPoint {
        name: "公開データ読み取り接続",
        source: trace::Source {
            path: "crates/game-record-persistence/src/lib.rs",
            needle: "pub async fn connect(url: &str) -> Result<Self, Error> {"
        }
    }]
});
