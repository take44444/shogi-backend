use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "チケット応答はサーバー状態を保持し、新規作成・再試行・拒否・組合せ済みチケットの取消を区別する。ルーターのミドルウェアが no-store を追加する。",
    source: trace::Source {
        path: "crates/realtime-gateway/src/matchmaking.rs",
        needle: "fn ticket_response("
    },
    gap: Some(
        "no-store は代表的な成功と Origin の失敗で確認するが、全応答区分は扱わない。DTO のすべての破損と上流エラーの変換には、専用の検証が不足している。"
    ),
});
