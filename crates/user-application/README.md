# user-application の参照仕様

このクレートはアカウントの型と非同期の `AccountRepository` ポートを定義する。
プロフィールは共有の `UserId`、`DisplayName`、作成時刻を含む。
外部 ID は検証済みの発行者とサブジェクトを使う。ログイン試行とセッションは有効期限を持つ。

## 検証と責務

`DisplayName::new` は前後の空白を取り除く。
空の名前、80 Unicode 文字を超える名前、制御文字を含む名前は拒否する。
`LoginAttempt`、`VerifiedIdentity`、`NewSession` は認証データを持つため、`Debug` を実装しない。

[HTTP アダプター](../user-http/README.md)は、リポジトリを呼び出す前に Google の本人確認情報を検証する。
リポジトリはログイン試行を原子的に消費し、アカウントの特定とセッション作成を 1 つのトランザクションで行う。
[PostgreSQL アダプター](../user-persistence/README.md)がこれらの操作を実装する。
このクレートは対局、HTTP、データベース、OAuth のライブラリーに依存しない。

## テスト範囲

単体テストは表示名の検証を対象とする。
リポジトリのトランザクションと有効期限の動作は `user-persistence` で検証する。
実行コマンドは[ワークスペースの検証手順](../../README.md#ワークスペースを検証する)に記述する。
