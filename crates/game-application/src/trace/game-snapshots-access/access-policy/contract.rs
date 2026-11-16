use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "公開状態または固定された参加者への所属によってアクセスを許可する。CheckViewAccess は履歴を読み込まずメタデータを使う。",
    source: trace::Source {
        path: "crates/game-application/src/lib.rs",
        needle: "pub async fn check_view_access("
    },
    gap: Some(
        "アクセス可否だけを調べる処理が履歴の読み込みを省くことを示す問い合わせ回数の検証はなく、ストレージやアクセス情報の破損分岐もすべては列挙していない。"
    ),
});
