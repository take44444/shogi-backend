use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "合成した最大値でシーケンス補助関数を直接検証する。ルール実行や DB は使わない。",
        reason: "直接呼び出した補助関数で、移植可能な最大値と使い切った境界を検証する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn sequence_cap_is_portable() {
    assert_eq!(next_sequence(i64::MAX as u64 - 1), Ok(i64::MAX as u64));
    assert_eq!(
        next_sequence(i64::MAX as u64),
        Err(ApplicationError::SequenceExhausted)
    );
    assert_eq!(
        next_sequence(u64::MAX),
        Err(ApplicationError::SequenceExhausted)
    );
}
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "エラーの優先順位を確認するため、アクターの状態を最大シーケンスに直接設定する。",
        reason: "合成した上限到達済みのアクターで、古いシーケンスに対する SequenceMismatch と SequenceExhausted を区別する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn exhausted_state_still_checks_expected_sequence_before_advancing() {
    let repository = store();
    let mut actor = Actor::new(id(), repository.clone());
    actor.refresh().await.unwrap();
    actor.state.as_mut().unwrap().seq = i64::MAX as u64;
    repository.0.lock().unwrap().version = i64::MAX as u64;
    let mut request = receipt();
    request.expected_seq = u64::MAX;
    assert_eq!(
        actor.apply(request.clone()).await,
        Err(ApplicationError::SequenceMismatch {
            expected: u64::MAX,
            actual: i64::MAX as u64
        })
    );
    request.expected_seq = i64::MAX as u64;
    assert_eq!(
        actor.apply(request).await,
        Err(ApplicationError::SequenceExhausted)
    );
}
}
