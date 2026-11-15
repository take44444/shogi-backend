use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "user HTTP の cookie は対象 Cookie を全 Cookie ヘッダーから探し、重複・非 hex・64 文字以外・欠落を Unauthorized とする。GET /me は受け取った値の SHA-256 だけを repository に渡す。",
    source: trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "fn cookie<'a>(headers:"
    },
    gap: Some(
        "user HTTP 自体の複数 Cookie ヘッダー、非 hex と長さ境界は直接確認していない。realtime の Cookie 検証は別実装である。"
    ),
});
