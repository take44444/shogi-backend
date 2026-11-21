fn change(metadata: &mut game_record_application::Metadata) {
    metadata.final_seq = game_record_application::OperationSequence::try_from(2u64).unwrap();
}
