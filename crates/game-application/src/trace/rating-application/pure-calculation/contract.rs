use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "外部依存のない計算処理は検証済み RatingState を使い、実効開始値から丸めた K32 差分を一つ計算する。その符号を反転した値を後手へ適用し、レーティングと対局数のオーバーフローを検査する。",
    source: trace::Source {
        path: "crates/rating-domain/src/lib.rs",
        needle: "pub fn calculate("
    },
    gap: Some(
        "これらの有限の事例は、数学的な証明でも、浮動小数点の丸めの移植性を網羅的に保証するものでもない。"
    ),
});
