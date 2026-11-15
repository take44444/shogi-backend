use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "logout は Origin の完全一致と、存在する場合の Sec-Fetch-Site: same-origin を要求する。有効な形式の Cookie は repository から削除し、欠落や不正形式でも Cookie を消去して 204 を返す。",
    source: trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "async fn logout("
    },
    gap: Some(
        "ログアウトの hostile Origin、cross-site の Sec-Fetch-Site、不正形式 Cookie、repository 削除失敗を直接確認する assertion はない。"
    ),
});
