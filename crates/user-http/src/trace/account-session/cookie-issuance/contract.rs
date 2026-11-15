use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "set_cookie は Domain を付けず Path=/・HttpOnly・SameSite=Lax・指定 Max-Age を使い、secure 構成では Secure を付ける。BrowserConfig は本番と開発用の Cookie 名を選ぶ。",
    source: trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "fn set_cookie("
    },
    gap: Some(
        "本番のセッション Cookie の属性と Max-Age、ログアウトの Max-Age=0 を直接比較する assertion はない。ログイン Cookie と共有する helper の証拠を本番セッションの直接検証と扱わない。"
    ),
});
