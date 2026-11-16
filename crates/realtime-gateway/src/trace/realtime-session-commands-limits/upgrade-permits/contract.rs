use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "アップグレードでは認証・アクセス確認を行い、上流の呼び出しの前後で停止状態を確認する。接続全体とユーザーごとの枠を取得し、フレーム・メッセージの上限を設定する。切断時のDropで枠を解放する。",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "async fn upgrade("
    },
    gap: Some(
        "上流とのハンドシェイク中の停止を扱う専用の競合検証はない。接続枠のテストは、処理中のハンドシェイクの全状態を網羅しない。"
    ),
});
