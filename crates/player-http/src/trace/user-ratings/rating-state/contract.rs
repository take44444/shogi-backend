use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "RatingState は未評価を null・0、評価済みを整数レートと正の適用済み対局数として表現する。",
    source: trace::Source {
        path: "crates/rating-domain/src/lib.rs",
        needle: "pub enum RatingState {"
    },
    gap: Some(
        "不正な組み合わせの全列挙や読み取り側の破損値拒否は、この機能のテストでは確認していない。ドメイン全体のテスト一覧は対象外。"
    ),
});
