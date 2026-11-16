use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "対局ごとの操作窓口は、操作 64 件の有界チャネルを使う。応答の受信側が破棄されてもアクターは実行を続け、閉じたキュー内の操作を処理して終了する。",
    source: trace::Source {
        path: "crates/game-application/src/lib.rs",
        needle: "const MAILBOX_CAPACITY: usize = 64;"
    },
    gap: Some(
        "64 件の上限は処理中の操作と送信待ちの呼び出しを除いたキュー内の件数に適用する。既知の対局を保持するアクター登録簿全体の上限や公平性は保証していない。"
    ),
});
