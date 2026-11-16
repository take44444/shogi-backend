use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "読み取り処理は、フレーム・頻度の上限、厳密なコマンド解析、同時実行コマンド枠1件、上限付きキューを適用する。解析処理は、未知のフィールドと、正規形でない10進連番・USIを拒否する。",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "async fn reader("
    },
    gap: Some(
        "不正な10進文字列、範囲外の数値、未知の全フィールドを列挙する解析テストはない。処理中・同時実行枠、コマンドキュー・出力の上限超過を個別に検証するテストもない。"
    ),
});
