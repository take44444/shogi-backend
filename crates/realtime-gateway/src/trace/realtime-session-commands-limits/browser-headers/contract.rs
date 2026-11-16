use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "browser_headersはOriginとHostがそれぞれ1件で完全一致することを求める。fetch-siteの欠落またはsame-originだけを許可し、対応するshogi.realtime.v1の提示をちょうど1件検出する。",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "fn browser_headers(headers: &HeaderMap, origin: &str) -> bool {"
    },
    gap: Some(
        "選択したテストでは、Host、サブプロトコルの重複、ヘッダーの正規化のすべての組み合わせを明示的には検証しない。"
    ),
});
