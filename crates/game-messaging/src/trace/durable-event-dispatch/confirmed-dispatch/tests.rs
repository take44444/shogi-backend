use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "TCP上のRESP応答を返すテスト用サーバー、実際のRedisクライアントとValkeyEventPublisher、テスト用Outboxを使う。稼働中のValkeyは使わない。",
        reason: "TCP上のRESP応答を返すテスト用サーバーと実際のRedisクライアントを使い、配信済みの記録が応答を待つこと、購読者数0は受け入れて不正な応答は拒否することを確認する。",
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
        scope: "TCP上のRESP応答を返すテスト用サーバーを使い、キャンセル時に接続を再利用せず、再接続もしないことを確認する。",
        reason: "配信をキャンセルすると、接続を再利用できず、自動再接続もしない。",
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
        scope: "TCP上のRESP応答を返すテスト用サーバーを使い、配信応答のタイムアウトと未配信状態を確認する。",
        reason: "応答がタイムアウトしても、Outboxの未配信イベントを保持する。",
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
