use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "http-user-ratings",
    consumer: "player-http",
    provider: "game-record-application",
    interface: trace::Interface::RustApi,
    statement: "UserRatings::get は型付きユーザー ID を RatingRepository に渡し、その結果を返す。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/game-record-application/src/ratings.rs",
        needle: "pub async fn get(&self, user: &UserId) -> Result<UserRatingSnapshot, Error> {"
    },
    call: Some(trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "async fn ratings(State(app): State<App>, path: Result<Path<String>, PathRejection>) -> Response {"
    }),
    implementation: Some(trace::Source {
        path: "crates/game-record-application/src/ratings.rs",
        needle: "pub async fn get(&self, user: &UserId) -> Result<UserRatingSnapshot, Error> {"
    }),
    binding: None,
    requires: &[trace::Dependency {
        contract: "game-record-application/rating-store",
        reason: "提供元はこの直接の依存先を利用して、上位の応答と状態の保証を満たす。"
    }],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
