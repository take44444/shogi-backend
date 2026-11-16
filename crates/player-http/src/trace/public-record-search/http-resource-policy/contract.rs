use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "共有アダプターはセマフォの許可を即座に取得する。respond はエラーを変換し、no-store を付け、シリアライズした JSON を 1 MiB 以内に制限する。",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "fn respond<T: Serialize>(result: Result<T, Error>) -> Response {"
    },
    gap: Some(
        "サイズの境界には実際の最大ページではなく文字列を使う。検索・詳細取得・レート取得を混在させて処理枠を使い切るテストは対応付けていない。"
    ),
});
