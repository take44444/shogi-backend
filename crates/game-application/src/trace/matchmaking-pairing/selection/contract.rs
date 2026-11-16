use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "選択処理は、期限切れの待機、同じ所有者、別プールを除外する。基準側をキュー追加時刻・UUID・所有者の順、相手側を差・時刻・UUID・所有者の順に並べる。",
    source: trace::Source {
        path: "crates/game-application/src/matchmaking.rs",
        needle: "pub fn select_pair(tickets: &[Ticket], now: i64) -> Option<(&Ticket, &Ticket)> {"
    },
    gap: None,
});
