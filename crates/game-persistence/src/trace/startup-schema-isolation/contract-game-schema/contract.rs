use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "書き込み側の初期化処理は、アドバイザリートランザクションロックの下で、空のスキーマを作成し、必須定義の指紋とバージョン 7 を確認する。既存の読み取り用ロールに四つの公開テーブルの権限を付与する。",
    source: trace::Source {
        path: "crates/game-persistence/src/schema.rs",
        needle: "pub(crate) async fn initialize(db: &DatabaseConnection) -> Result<(), SchemaError> {"
    },
    gap: Some(
        "検証したスキーマとデータのスナップショットが保存されても、すべての SQL 副作用がないことは示さない。書き込み側の検証は一時的な期待スキーマを作り、読み取り役に SELECT を付与する場合がある。読み取り側の列の検証方針は、書き込み側の部分集合の検証より厳しい。"
    ),
});
