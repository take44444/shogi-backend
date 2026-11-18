use crate::trace_support as trace;
trace::component!(trace::Component {
    description: "user-application コンポーネント",
    notes: &[
        "account-profile/display-name。DisplayName::new は trim 後の文字列を保持し、空文字・80 Unicode 文字超過・制御文字を InvalidInput とする。"
    ],
    external: &[],
    operations: &[],
    entrypoints: &[]
});
