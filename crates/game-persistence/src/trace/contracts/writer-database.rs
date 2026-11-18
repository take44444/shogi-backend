use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "writer-database",
    consumer: "game-persistence",
    provider: "postgres",
    interface: trace::Interface::External,
    statement: "スキーマ初期化と保存操作にトランザクションと制約を適用する。未対応の既存スキーマを変更せず拒否し、失敗したトランザクションの変更を残さない。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/game-persistence/src/lib.rs",
        needle: "pub async fn connect(url: &str) -> Result<Self, Error> {"
    },
    call: Some(trace::Source {
        path: "crates/game-persistence/src/lib.rs",
        needle: "pub async fn connect(url: &str) -> Result<Self, Error> {"
    }),
    implementation: None,
    binding: None,
    requires: &[],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
