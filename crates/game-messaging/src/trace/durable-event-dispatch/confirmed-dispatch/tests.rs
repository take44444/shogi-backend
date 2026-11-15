use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "TCP RESP fixture plus actual Redis client/ValkeyEventPublisher and fake outbox, no deployed Valkey.",
        reason: "A TCP RESP fixture and actual Redis client show marking waits for the response and accepts zero subscribers but rejects invalid responses.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
    async fn outbox_marks_only_after_a_valid_publish_response_including_no_subscribers() {
        for (reply, accepted) in [
            (b":0\r\n".as_slice(), true),
            (b":3\r\n".as_slice(), true),
            (b"-NOPERM publish denied\r\n".as_slice(), false),
            (b"+OK\r\n".as_slice(), false),
            (b"$1\r\n0\r\n".as_slice(), false),
            (b":-1\r\n".as_slice(), false),
            (b"".as_slice(), false),
        ] {
            let (url, seen, release, server) = server(reply).await;
            let transport = ValkeyEventPublisher::connect(&url).await.unwrap();
            let marked = Arc::new(AtomicBool::new(false));
            let queue = Queue(marked.clone());
            let mut job =
                tokio::spawn(
                    async move { OutboxPublisher::new(queue, transport).publish_next().await },
                );
            seen.await.unwrap();
            assert!(
                tokio::time::timeout(Duration::from_millis(25), &mut job)
                    .await
                    .is_err()
            );
            assert!(!marked.load(Ordering::SeqCst));
            release.send(()).unwrap();
            assert_eq!(job.await.unwrap().is_ok(), accepted);
            assert_eq!(marked.load(Ordering::SeqCst), accepted);
            server.await.unwrap();
        }
    }
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "TCP RESP fixture checks cancellation and absence of connection reuse/reconnect.",
        reason: "Cancelling publication prevents connection reuse or automatic reconnection.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
    async fn cancelled_publication_cannot_reuse_a_connection_or_reconnect() {
        let (url, seen, release, server) = server(b":0\r\n").await;
        let mut publisher = ValkeyEventPublisher::connect(&url).await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), publisher.publish(&event()))
                .await
                .is_err()
        );
        seen.await.unwrap();
        assert!(publisher.publish(&event()).await.is_err());
        release.send(()).unwrap();
        server.await.unwrap();
    }
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "TCP RESP fixture checks publication response timeout and pending state.",
        reason: "Response timeout preserves the pending outbox event.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
    async fn publication_timeout_leaves_the_outbox_pending() {
        let (url, seen, release, server) = server(b":0\r\n").await;
        let transport = ValkeyEventPublisher::connect(&url).await.unwrap();
        let marked = Arc::new(AtomicBool::new(false));
        let queue = Queue(marked.clone());
        let job =
            tokio::spawn(
                async move { OutboxPublisher::new(queue, transport).publish_next().await },
            );
        seen.await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(6), job)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        assert!(!marked.load(Ordering::SeqCst));
        release.send(()).unwrap();
        server.await.unwrap();
    }
}
