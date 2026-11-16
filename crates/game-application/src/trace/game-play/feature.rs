use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "着手・将棋ルール・詰み・投了",
    source: trace::Source {
        path: "crates/shogi-domain/src/lib.rs",
        needle: "pub fn play(&mut self, player: Player, mv: Move) -> Result<MoveOutcome, CommandError> {"
    },
    flow: &[
        "MakeMove と Resign は通信上の ID と必須の予想シーケンスを検証する。着手入力はアプリケーションに入る前に厳密な USI として解析する。",
        "アクターは不変の参加者割り当てから先後を導出し、複製した対局をステージする。ドメインのルールは合法手だけを適用し、エラー時には観測可能な状態を保持する。",
        "ルールアダプターは、成り・駒打ち・玉の安全の制限を含む合法手を生成する。ドメイン処理は詰みまたは参加者の投了を認識し、終局局面を保持する。",
        "受理候補の状態は game-command-recovery で追跡する受理記録・イベントの原子的なコミット処理へ進み、RPC 変換は安定した受理・拒否のステータスを公開する。"
    ],
    guarantees: &[
        trace::Guarantee {
            id: "participant-input",
            statement: "割り当てられた参加者だけが変更できる。先後は固定された割り当てから決まり、着手は厳密な USI を使う。不正な通信上の値は対局変更前に失敗する。",
            contracts: &["rpc-move", "rpc-resign", "binding", "notation"]
        },
        trace::Guarantee {
            id: "shogi-rules",
            statement: "合法手処理は、すべての通常駒・成駒の移動、移動を遮る駒、捕獲と持ち駒への生駒化、駒打ち、二歩、行き所のない駒の制限、任意・強制の成り、玉の安全と王手回避、打ち歩詰めの拒否に対応する。",
            contracts: &["legal-moves", "domain-play"]
        },
        trace::Guarantee {
            id: "terminal",
            statement: "詰みは詰ませた側を勝者として対局を終了する。王手でないときに合法手がなくても敗北とはしない。どちらの側も手番外で投了でき、相手が勝者となる。終局後は新しい着手と投了を拒否し、合法手を返さない。",
            contracts: &["domain-play", "domain-resign", "rpc-errors"]
        },
        trace::Guarantee {
            id: "atomic-rejection",
            statement: "ドメインが操作を拒否すると対局全体を保持し、アプリケーションは拒否した操作による局面変更をコミット済みとして公開しない。ルールエンジンの失敗は通常の非合法手と区別する。",
            contracts: &["legal-moves", "domain-play", "rpc-errors"]
        },
        trace::Guarantee {
            id: "adjudication-scope",
            statement: "千日手や連続王手の局面でも対局を続行できる。時計、時間切れ、入玉・宣言、千日手の裁定はない。プール名はこれらを有効にしない。",
            contracts: &["domain-play"]
        }
    ],
    limitations: &[
        "対応するすべてのルール系統には、テスト用局面による直接の根拠がある。選択した実ネットワークテストが扱うのは代表的な合法手順と投了であり、全ルールや詰みに至る全手順ではない。",
        "SFEN からの構築はテスト用局面の準備専用であり、公開された取り込み・編集機能ではない。クライアントが任意に指定できる対局作成 RPC は提供しない。",
        "gRPC 呼び出し元の識別情報は信頼する申告値である。ブラウザー認証となりすましの拒否は realtime-session-commands-limits で追跡する。",
        "リンクとテスト成功だけでは意味上の完全性は成立しない。保証、根拠の強さ、分割の変更にはレビューが必要となる。"
    ],
    entrypoints: &[],
    surfaces: &[
        trace::Surface::Grpc {
            path: "proto/game.proto",
            service: "GameService",
            operation: "MakeMove"
        },
        trace::Surface::Grpc {
            path: "proto/game.proto",
            service: "GameService",
            operation: "Resign"
        }
    ],
});
