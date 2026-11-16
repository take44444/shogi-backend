use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "対局スナップショットと閲覧制御",
    source: trace::Source {
        path: "crates/game-application/src/lib.rs",
        needle: "pub async fn snapshot_for_viewer("
    },
    flow: &[
        "GetSnapshot は対局ごとのアクターへ処理を渡す。アクターは永続バージョンを読み、変更または無効化された状態を、検証済み受理記録から復元する。",
        "GetSnapshotForViewer はアクセス用メタデータと履歴を整合した形で読み取る。CheckViewAccess は履歴を要求せず、現在のアクセス用メタデータを読む。",
        "GameRpc はアプリケーションが所有するスナップショットを、盤上の駒、すべての持ち駒カウンター、合法 USI 手、固定された参加者、手番・王手・結果・シーケンスへ変換する。ブラウザーゲートウェイは閲覧者の申告を別途認証する。"
    ],
    guarantees: &[
        trace::Guarantee {
            id: "snapshot-shape",
            statement: "信頼済み呼び出し元へのスナップショット取得が成功すると、永続シーケンス、固定された参加者、所有する盤面と 14 個の持ち駒カウンター、手番・王手・結果・合法手を返す。終局後も局面を保持し、合法手は空となる。",
            contracts: &["snapshot-refresh", "owned-position", "grpc-shape"]
        },
        trace::Guarantee {
            id: "viewer-policy",
            statement: "公開対局はゲートウェイ経由の認証済み観戦者を許可する。非公開対局は参加者を許可する。閲覧者向けスナップショット・アクセス RPC は、アクセス不能な対局と存在しない対局を同じ NotFound として扱う。",
            contracts: &["viewer-read", "access-policy", "grpc-viewer"]
        },
        trace::Guarantee {
            id: "recovery",
            statement: "スナップショットは永続バージョンを更新し、読み取り失敗時には状態を返さず、破損した履歴やメタデータを拒否する。再起動時には進行中と終局後のスナップショットを復元する。",
            contracts: &["snapshot-refresh", "restore", "durable-read"]
        }
    ],
    limitations: &[
        "信頼済み gRPC 呼び出し元のユーザー ID と閲覧者 ID は申告値であり、認証はゲートウェイが担う。GetSnapshot は内部の信頼済み読み取りである。",
        "閲覧者向け読み取り実装の各性質をすべて確認する、独立した問い合わせ回数の検証や、公開範囲と履歴の同時変更の検証はない。",
        "リンクとテスト成功だけでは意味上の完全性は成立しない。保証、根拠の強さ、分割の変更にはレビューが必要となる。"
    ],
    entrypoints: &[],
    surfaces: &[
        trace::Surface::Grpc {
            path: "proto/game.proto",
            service: "GameService",
            operation: "GetSnapshot"
        },
        trace::Surface::Grpc {
            path: "proto/game.proto",
            service: "GameService",
            operation: "GetSnapshotForViewer"
        },
        trace::Surface::Grpc {
            path: "proto/game.proto",
            service: "GameService",
            operation: "CheckViewAccess"
        }
    ],
});
