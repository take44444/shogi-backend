use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "verify_response はライブラリーの署名・issuer・audience・nonce・期限検証に加えて azp と access-token hash を検証し、空でない subject と固定 Google issuer を返す。",
    source: trace::Source {
        path: "crates/user-http/src/google.rs",
        needle: "fn verify_response("
    },
    gap: Some(
        "空 subject の拒否、ID token 欠落、名前がない場合や不正な名前の場合の Player へのフォールバックはこの記録のテストでは直接確認していない。"
    ),
});
