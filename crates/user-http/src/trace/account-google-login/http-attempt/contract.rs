use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "start は state とブラウザー値の SHA-256 と nonce・verifier を保存する。callback は一致する試行を消費した後に provider の交換を呼び、新セッションを保存して /me への 303 と Cookie を返す。",
    source: trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "async fn callback("
    },
    gap: Some(
        "callback の state/code 長境界、provider error、code 欠落、重複ログイン Cookie、保存失敗時の HTTP 応答は未確認。既存セッション Cookie を送ると旧セッションを logout する実装だが、そのローテーションを確認する assertion はない。"
    ),
});
