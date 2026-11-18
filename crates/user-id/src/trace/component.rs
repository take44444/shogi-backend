use crate::trace_support as trace;
trace::component!(trace::Component {
    description: "user-id コンポーネント",
    notes: &[
        "account-profile/user-id。UserId::new は空でなく 128 UTF-8 バイト以内で Unicode 空白を含まない内部 ID を保持する。",
        "user-ratings/user-id。UserId は 1〜128 UTF-8 バイトで Unicode 空白を含まない ID を保持する。"
    ],
    external: &[],
    operations: &[],
    entrypoints: &[]
});
