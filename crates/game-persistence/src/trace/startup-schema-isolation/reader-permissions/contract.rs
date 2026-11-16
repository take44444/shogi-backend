use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "用意した shogi_reader ロールは schema_version、games、journal、rating_states を SELECT できる。レーティング・組合せの内部情報にはアクセスできず、既定の読み取り専用設定を無効にしても書き込めない。",
    source: trace::Source {
        path: "scripts/init-postgres.sql",
        needle: "CREATE ROLE shogi_reader LOGIN PASSWORD 'shogi_reader';"
    },
    gap: Some(
        "これらの検証は、テストで用意したロールを使う。任意の本番環境における認証情報の所有権や権限方針を証明するものではない。"
    ),
});
