use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "snapshot_for_viewer は、リポジトリの一つのアクセス情報・履歴の読み取り結果を使う。アクセス不能や対局の識別子不一致を拒否し、所有するスナップショットを復元する。",
    source: trace::Source {
        path: "crates/game-application/src/lib.rs",
        needle: "pub async fn snapshot_for_viewer("
    },
    gap: Some(
        "公開範囲と履歴を同時に変更する専用テストはなく、閲覧者読み取り全体の整合性の契約を独立には確認していない。"
    ),
});
