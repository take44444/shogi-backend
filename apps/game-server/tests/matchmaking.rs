use game_application::matchmaking::TicketId;
use game_grpc::proto::{self, game_service_client::GameServiceClient};
use game_persistence::SeaOrmStore;
use pg_test_support::TestDatabase;
use sea_orm::{ConnectionTrait, Database};
use std::{
    process::{Child, Command, Stdio},
    time::Duration,
};

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

include!(
    "trace/matchmaking-pairing/periodic/writer_periodic_matching_progresses_without_another_join.rs"
);

#[cfg(test)]
#[path = "../../../scripts/traceability_model.rs"]
mod trace_support;
