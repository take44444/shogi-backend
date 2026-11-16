use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "成否不明の結果も含め、テスト用キューと通信先を使うアプリケーションの配信処理。",
        reason: "テスト用キューと通信先を使い、送信失敗と成否不明の処理済み更新の後も、重複配信を含めた配信シーケンス順を検証する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
    async fn failed_publish_and_uncertain_mark_preserve_identity_and_order() {
        for (send_failure, mark_failure, committed_mark, expected) in [
            (true, false, false, vec![1, 1, 2]),
            (false, true, false, vec![1, 1, 2]),
            (false, true, true, vec![1, 2]),
        ] {
            let first =
                StagedGameChange::new(game(), receipt(Action::Move("7g7f".parse().unwrap()), None))
                    .unwrap()
                    .events()[0]
                    .clone();
            let mut second = first.clone();
            second.seq = 2;
            let observed = Arc::default();
            let queue = Queue {
                events: [first, second].into(),
                mark_failure,
                committed_mark,
            };
            let sink = Sink {
                observed: Arc::clone(&observed),
                fail: send_failure,
            };
            let mut runner = OutboxPublisher::new(queue, sink);
            assert!(runner.publish_next().await.is_err());
            while runner.publish_next().await.unwrap() {}
            assert_eq!(*observed.lock().unwrap(), expected);
        }
    }
}
