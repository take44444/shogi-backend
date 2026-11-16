use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "shutdownは持続する停止通知を保存する。稼働中の接続はその通知を監視し、アップグレードは新たな処理を拒否する。wait_closedは接続全体の枠が解放されるまでの待機時間を制限する。",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "pub fn shutdown(&self) {"
    },
    gap: Some(
        "テストはルーター・ライブラリーの停止処理を通す。実行ファイルへのCtrl-C、設定の検証、10秒の終了待機期限は確認しない。wait_closedの待機期限切れも個別には検証しない。"
    ),
});
