use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "公開対局、ユーザー、マッチングの各サンプルの入口は、ライブラリーの文書を整形済み JSON としてシリアライズする。",
    source: trace::Source {
        path: "crates/player-http/examples/player-openapi.rs",
        needle: "fn main() -> Result<(), Box<dyn std::error::Error>> {"
    },
    gap: Some(
        "対応付けたテストはサンプルのプロセスを起動せず、その標準出力、生成用シェルスクリプト、公開済みファイルも確認しない。同等のサンプルは crates/user-http/examples/user-openapi.rs と crates/realtime-gateway/examples/matchmaking-openapi.rs にある。"
    ),
});
