use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "AuthClient requires one correctly shaped cookie, calls the user service with response size/time limits and recheck requires the returned user to remain the session user.",
    source: trace::Source {
        path: "crates/realtime-gateway/src/auth.rs",
        needle: "pub(crate) async fn authenticate(&self, cookie: &str) -> Result<UserId, AuthError> {"
    },
    gap: Some(
        "No dedicated assertion covers auth service outage/redirect/oversized profile, returned user mismatch or every cookie encoding branch."
    ),
});
