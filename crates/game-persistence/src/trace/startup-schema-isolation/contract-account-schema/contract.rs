use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "PostgresAccounts::connect はスキーマの初期化と検証を直列化し、互換データを保持したまま、必須アカウント定義や台帳の欠落・変更を拒否する。",
    source: trace::Source {
        path: "crates/user-persistence/src/lib.rs",
        needle: "    pub async fn connect(url: &str) -> Result<Self, Error> {"
    },
    gap: Some(
        "選択した変更操作は、考えられるすべての DDL の差異ではない。アカウント実行時の処理、プロバイダー情報の取得、シグナル停止は、永続層の事例では検証しない。"
    ),
});
