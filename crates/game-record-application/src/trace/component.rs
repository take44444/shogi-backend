use crate::trace_support as trace;
trace::component!(trace::Component {
    description: "game-record-application コンポーネント",
    notes: &[
        "public-record-detail/application-forwarding。GameRecords::detail は型付き GameId をリポジトリに渡し、その結果を返す。 未検証範囲。任意の ID と結果が変更なしで転送されることを検証する独立したテストはない。",
        "public-record-detail/metadata-invariants。メタデータは、異なる参加者、終了時のシーケンス番号、保存層のスカラー値として有効な境界、および着手・投了に応じた終了時の手数と操作数の関係を要求する。",
        "public-record-search/application-forwarding。GameRecords::search は型付き Search を RecordRepository に渡し、その結果を返す。 未検証範囲。任意の Search とリポジトリの結果が変更なしで転送されることを検証する独立したテストはない。",
        "public-record-search/bounds-cursor。Search は期間、取得件数、手数の境界を検証する。カーソルはページの取得件数ではなく、フィルターのフィンガープリントに結び付けて復号する。 未検証範囲。対応付けた事例は、フィンガープリントの全フィールドの変更や、カーソルのバイト数と UTF-8 の全境界を網羅しない。",
        "user-ratings/application-forwarding。UserRatings::get は型付きユーザー ID を RatingRepository に渡し、その結果を返す。 未検証範囲。任意の ID と結果の無変更転送を独立して確認するテストはない。HTTP の失敗変換では元の失敗種別の保持を確認できない。"
    ],
    external: &[],
    operations: &[],
    entrypoints: &[]
});
