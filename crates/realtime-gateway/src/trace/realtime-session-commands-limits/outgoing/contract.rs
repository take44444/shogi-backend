use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "書き込み処理は各フレームの前にセッション・アクセスを再確認し、送信・切断の待機時間を制限する。初期化時は、設定されたバイト数を超えるスナップショットを拒否し、上限付きの出力・通知チャネルを使う。",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "async fn writer("
    },
    gap: Some(
        "遅いブラウザーによる送信待ちの蓄積、出力書き込みの期限切れ、通知キューの枯渇、スナップショットの上流呼び出しのタイムアウトを検証するテストはない。スナップショットの上限は、この根拠が処理を通すrun_connectionで適用する。"
    ),
});
