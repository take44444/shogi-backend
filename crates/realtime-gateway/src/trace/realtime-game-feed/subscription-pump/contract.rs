use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "通知の転送処理は、上限付きキューを通して購読結果を渡し、障害やキューの枯渇で停止する。run_connectionは通知の枯渇・欠落を接続終了の条件として扱う。",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "async fn pump("
    },
    gap: Some(
        "選択した検証には、ゲートウェイの通知転送キューを直接満杯にするものはない。遅いブラウザーによる送信待ちの蓄積と書き込みのタイムアウトにも専用の検証はない。"
    ),
});
