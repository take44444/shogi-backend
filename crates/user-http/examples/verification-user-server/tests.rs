use super::*;
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use pg_test_support::TestDatabase;
use tower::ServiceExt;

#[tokio::test]
async fn real_accounts_bind_callbacks_to_browser_and_persist_sessions_across_provider_restart() {
    let database = TestDatabase::new().await;
    let store = Arc::new(PostgresAccounts::connect(database.url()).await.unwrap());
    let origin = "http://localhost:8080";
    let provider = Arc::new(provider::VerificationProvider::new(origin.into()));
    let app = user_http::router(
        store.clone(),
        provider.clone(),
        BrowserConfig::new(origin, true).unwrap(),
    )
    .merge(provider::router(provider.clone()));
    let start = app
        .clone()
        .oneshot(
            Request::get("/auth/google/start")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let login = start.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let authorization = start.headers()[header::LOCATION].to_str().unwrap();
    let request = url::Url::parse(authorization)
        .unwrap()
        .query_pairs()
        .next()
        .unwrap()
        .1
        .into_owned();
    for (actor, origin, status) in [
        ("alice", "http://foreign.invalid", StatusCode::FORBIDDEN),
        ("mallory", origin, StatusCode::UNPROCESSABLE_ENTITY),
    ] {
        let selection = app
            .clone()
            .oneshot(
                Request::post("/__verify/authorize")
                    .header(header::ORIGIN, origin)
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(format!("request={request}&actor={actor}")))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(selection.status(), status);
    }
    let selection = app
        .clone()
        .oneshot(
            Request::post("/__verify/authorize")
                .header(header::ORIGIN, origin)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!("request={request}&actor=alice")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(selection.status(), StatusCode::SEE_OTHER);
    let callback = selection.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_owned();
    let other_start = app
        .clone()
        .oneshot(
            Request::get("/auth/google/start")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let other_login = other_start.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    assert_ne!(login, other_login);
    let wrong = app
        .clone()
        .oneshot(
            Request::get(&callback)
                .header(header::COOKIE, other_login)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
    let success = app
        .clone()
        .oneshot(
            Request::get(&callback)
                .header(header::COOKIE, &login)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(success.status(), StatusCode::SEE_OTHER);
    let session = success
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .find_map(|h| {
            let value = h.to_str().unwrap();
            value
                .starts_with("user_session=")
                .then(|| value.split(';').next().unwrap().to_owned())
        })
        .unwrap();
    let replay = app
        .oneshot(
            Request::get(&callback)
                .header(header::COOKIE, login)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
    let restarted = user_http::router(
        store.clone(),
        Arc::new(provider::VerificationProvider::new(origin.into())),
        BrowserConfig::new(origin, true).unwrap(),
    );
    let profile = restarted
        .oneshot(
            Request::get("/me")
                .header(header::COOKIE, session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(profile.status(), StatusCode::OK);
    store.close().await.unwrap();
}
