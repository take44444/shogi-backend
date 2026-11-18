use crate::trace_support as trace;
trace::component!(trace::Component {
    description: "user-persistence コンポーネント",
    notes: &[
        "account-google-login/account-session-transaction。PostgresAccounts::login は issuer・subject のロック下で既存プロフィールを再利用するかアカウントと外部 ID を作成し、セッション挿入と一緒にコミットする。 未検証範囲。issuer だけが異なる場合の分離や、コミット時失敗などすべてのトランザクション障害は列挙していない。",
        "account-google-login/attempt-consumption。PostgresAccounts::consume_attempt は state・browser の両ハッシュが一致し expires_at > now の行を DELETE RETURNING で一度だけ取得する。",
        "account-profile/existing-identity。PostgresAccounts::login は既存 issuer・subject の Profile を再利用し、provider の initial_name で既存表示名を置き換えない。",
        "account-profile/profile-update。PostgresAccounts::rename は有効期限内のセッションからアカウントを解決し、表示名更新をトランザクションで保存して更新済み Profile を返す。 未検証範囲。名前更新の保存失敗・コミット失敗・別アカウントへの非干渉は直接確認していない。",
        "account-session/session-storage。auth_query は token_hash と expires_at > now で session と account を結合する。HTTP が保存するセッションの識別値は生の Cookie ではなく SHA-256 となる。 未検証範囲。破損プロフィールの decode と保存層障害の HTTP 503 変換はこの記録のテストで直接確認していない。",
        "startup-schema-isolation/contract-account-schema。PostgresAccounts::connect はスキーマの初期化と検証を直列化し、互換データを保持したまま、必須アカウント定義や台帳の欠落・変更を拒否する。 未検証範囲。選択した変更操作は、考えられるすべての DDL の差異ではない。アカウント実行時の処理、プロバイダー情報の取得、シグナル停止は、永続層の事例では検証しない。"
    ],
    external: &[],
    operations: &[],
    entrypoints: &[trace::EntryPoint {
        name: "ユーザー DB の初期化とスキーマ検査",
        source: trace::Source {
            path: "crates/user-persistence/src/lib.rs",
            needle: "pub async fn connect(url: &str) -> Result<Self, Error> {"
        }
    }]
});
