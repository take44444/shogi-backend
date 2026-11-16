use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "ゲートウェイは、注釈を付けたマッチングのパスと二つの Cookie セキュリティ方式を、オフライン OpenAPI 取得関数から出力する。",
    source: trace::Source {
        path: "crates/realtime-gateway/src/matchmaking.rs",
        needle: "pub(crate) fn openapi() -> utoipa::openapi::OpenApi {"
    },
    gap: Some(
        "このテストはゲートウェイの HTTP メソッドとパスの集合の存在を確認し、完全一致は確認しない。操作一覧の網羅性検査は、別途生成された操作と照合する。ゲートウェイに実行時の /openapi.json ルートはない。"
    ),
});
