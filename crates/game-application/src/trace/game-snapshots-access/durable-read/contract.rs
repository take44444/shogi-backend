use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "PostgreSQL は対局メタデータと順序付き受理記録を、一つの整合した読み取りで結合する。閲覧者向け読み取りは、その同じ結果に公開範囲のフィルターを適用する。",
    source: trace::Source {
        path: "crates/game-persistence/src/lib.rs",
        needle: "async fn load_view("
    },
    gap: Some(
        "通常の結合読み取りを並行する場合は検証している。閲覧者向けの公開範囲と履歴の同時変更は、独立には検証していない。"
    ),
});
