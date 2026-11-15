use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "PostgresAccounts::rename は有効期限内のセッションからアカウントを解決し、表示名更新をトランザクションで保存して更新済み Profile を返す。",
    source: trace::Source {
        path: "crates/user-persistence/src/lib.rs",
        needle: "    async fn rename(&self, hash:"
    },
    gap: Some("名前更新の保存失敗・コミット失敗・別アカウントへの非干渉は直接確認していない。"),
});
