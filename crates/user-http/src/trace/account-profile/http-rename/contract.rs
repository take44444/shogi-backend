use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "rename は serde の未知フィールド拒否を持つ JSON を抽出し、same_origin と DisplayName::new が成功した後、Cookie のハッシュに対応するプロフィールを repository で更新する。",
    source: trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "async fn rename("
    },
    gap: Some(
        "cross-site の Sec-Fetch-Site、正しい JSON・Origin に対する未認証 PATCH、HTTP を通した 80 文字・制御文字・trim 境界の assertion はない。"
    ),
});
