use super::*;
use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use tokio::sync::{Semaphore, mpsc};
use tower::ServiceExt;

struct Blocked {
    entered: mpsc::UnboundedSender<u32>,
    release: Semaphore,
}

#[async_trait::async_trait]
impl game_record_application::RecordRepository for Blocked {
    async fn search(&self, search: &Search) -> Result<Page, Error> {
        self.entered.send(search.limit()).unwrap();
        self.release.acquire().await.unwrap().forget();
        Ok(Page {
            items: vec![],
            next_cursor: None,
        })
    }

    async fn detail(&self, _: &GameId) -> Result<Detail, Error> {
        unreachable!()
    }
}

async fn request(app: Router, path: &str) -> Response {
    app.oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

include!(
    "trace/public-record-search/http-resource-policy/sixteen_active_requests_admit_no_seventeenth_and_release_capacity.rs"
);

include!(
    "trace/public-record-search/http-resource-policy/serialized_response_accepts_one_mib_and_rejects_the_next_byte.rs"
);
