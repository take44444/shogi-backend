use crate::trace_support as trace;
trace::component!(trace::Component {
    description: "rating-domain コンポーネント",
    notes: &[
        "rating-application/pure-calculation。外部依存のない計算処理は検証済み RatingState を使い、実効開始値から丸めた K32 差分を一つ計算する。その符号を反転した値を後手へ適用し、レーティングと対局数のオーバーフローを検査する。 未検証範囲。これらの有限の事例は、数学的な証明でも、浮動小数点の丸めの移植性を網羅的に保証するものでもない。",
        "user-ratings/rating-state。RatingState は未評価を null・0、評価済みを整数レートと正の適用済み対局数として表現する。 未検証範囲。不正な組み合わせの全列挙や読み取り側の破損値拒否は、この機能のテストでは確認していない。ドメイン全体のテスト一覧は対象外。"
    ],
    external: &[],
    operations: &[],
    entrypoints: &[]
});
