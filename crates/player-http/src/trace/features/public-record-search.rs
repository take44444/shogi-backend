use crate::trace_support as trace;
trace::feature!(trace::Feature {
    id: "public-record-search",
    title: "終了済み公開対局の検索",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "async fn search("
    },
    flow: &["フロントエンドの操作を受付境界で検証し、直接の依存先の契約を利用して応答する。"],
    guarantees: &[
        trace::Guarantee {
            id: "public-filtered-metadata",
            statement: "認証なしの検索は、取得時点で公開されている終了済み対局のメタデータだけを返し、指定されたフィルターを AND で組み合わせる。",
            roots: &["player-http/frontend-get-game-records"]
        },
        trace::Guarantee {
            id: "bounded-input",
            statement: "検索には、非負で開始より終了が後の UTC マイクロ秒区間を指定する。開始は含み終了は含まず、区間は最大 366 日となる。取得件数は 1〜100 で既定値は 30 とし、型付きフィルターに未知フィールドや重複フィールドを認めない。不正な入力は保存層へ到達する前に 400 を返す。",
            roots: &["player-http/frontend-get-game-records"]
        },
        trace::Guarantee {
            id: "seek-pages",
            statement: "結果は終了時刻、続いてバイナリー形式の対局 ID の降順に並ぶ。カーソルはフィルターに結び付き、ページの取得件数は変更できる。次のページでも公開状態を再確認する。",
            roots: &["player-http/frontend-get-game-records"]
        },
        trace::Guarantee {
            id: "resource-response-policy",
            statement: "処理対象の応答は詳細を伏せたエラーと no-store を使う。共有 HTTP アダプターは同時に最大 16 件を受け入れ、シリアライズ後の成功応答が 1 MiB を超える場合は拒否する。",
            roots: &["player-http/frontend-get-game-records"]
        }
    ],
    limitations: &[
        "下位の契約や内部テストの成功だけでは、この利用者向け保証の完全性を示さない。",
        "複数のページ要求で同じデータベーススナップショットを使う保証はない。カーソルはフィルターに結び付いた値であり、認証済みのトークンではない。",
        "対応付けたテストは実際の player-server の TCP リスナーを起動しない。フレームワーク自身が返す未処理ルートなどの応答は、処理対象の応答に対する no-store の保証に含まれない。",
        "読み取り接続数の上限、データベースのタイムアウト、データベース権限は startup-schema-isolation に対応付けている。",
        "ソースへのリンクとテストの実行成功は、追跡できる根拠を示す。保証の意味の証明や完全な網羅性を示すものではない。"
    ]
});
