use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "UserRatings::get は型付きユーザー ID を RatingRepository に渡し、その結果を返す。",
    source: trace::Source {
        path: "crates/game-record-application/src/ratings.rs",
        needle: "pub async fn get(&self, user: &UserId) -> Result<UserRatingSnapshot, Error> {"
    },
    gap: Some(
        "任意の ID と結果の無変更転送を独立して確認するテストはない。HTTP の失敗変換では元の失敗種別の保持を確認できない。"
    ),
});
