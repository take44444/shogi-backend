#[path = "trace/durable-event-dispatch/canonical-facts/contract.rs"]
mod trace_canonical_facts_contract;
#[path = "trace/durable-event-dispatch/confirmed-dispatch/contract.rs"]
mod trace_confirmed_dispatch_contract;
#[path = "trace/game-command-recovery/actor-apply/contract.rs"]
mod trace_contract_game_command_recovery_actor_apply;
#[path = "trace/game-command-recovery/canonical-events/contract.rs"]
mod trace_contract_game_command_recovery_canonical_events;
#[path = "trace/game-command-recovery/durable-commit/contract.rs"]
mod trace_contract_game_command_recovery_durable_commit;
#[path = "trace/game-command-recovery/mailbox/contract.rs"]
mod trace_contract_game_command_recovery_mailbox;
#[path = "trace/game-command-recovery/reconciliation/contract.rs"]
mod trace_contract_game_command_recovery_reconciliation;
#[path = "trace/game-command-recovery/replay/contract.rs"]
mod trace_contract_game_command_recovery_replay;
#[path = "trace/game-command-recovery/sequence-cap/contract.rs"]
mod trace_contract_game_command_recovery_sequence_cap;
#[path = "trace/game-play/binding/contract.rs"]
mod trace_contract_game_play_binding;
#[path = "trace/game-play/domain-play/contract.rs"]
mod trace_contract_game_play_domain_play;
#[path = "trace/game-play/domain-resign/contract.rs"]
mod trace_contract_game_play_domain_resign;
#[path = "trace/game-play/legal-moves/contract.rs"]
mod trace_contract_game_play_legal_moves;
#[path = "trace/game-play/notation/contract.rs"]
mod trace_contract_game_play_notation;
#[path = "trace/game-play/rpc-errors/contract.rs"]
mod trace_contract_game_play_rpc_errors;
#[path = "trace/game-play/rpc-move/contract.rs"]
mod trace_contract_game_play_rpc_move;
#[path = "trace/game-play/rpc-resign/contract.rs"]
mod trace_contract_game_play_rpc_resign;
#[path = "trace/game-snapshots-access/access-policy/contract.rs"]
mod trace_contract_game_snapshots_access_access_policy;
#[path = "trace/game-snapshots-access/durable-read/contract.rs"]
mod trace_contract_game_snapshots_access_durable_read;
#[path = "trace/game-snapshots-access/grpc-shape/contract.rs"]
mod trace_contract_game_snapshots_access_grpc_shape;
#[path = "trace/game-snapshots-access/grpc-viewer/contract.rs"]
mod trace_contract_game_snapshots_access_grpc_viewer;
#[path = "trace/game-snapshots-access/owned-position/contract.rs"]
mod trace_contract_game_snapshots_access_owned_position;
#[path = "trace/game-snapshots-access/restore/contract.rs"]
mod trace_contract_game_snapshots_access_restore;
#[path = "trace/game-snapshots-access/snapshot-refresh/contract.rs"]
mod trace_contract_game_snapshots_access_snapshot_refresh;
#[path = "trace/game-snapshots-access/viewer-read/contract.rs"]
mod trace_contract_game_snapshots_access_viewer_read;
#[path = "trace/matchmaking-pairing/durable-tick/contract.rs"]
mod trace_contract_matchmaking_pairing_durable_tick;
#[path = "trace/matchmaking-pairing/periodic/contract.rs"]
mod trace_contract_matchmaking_pairing_periodic;
#[path = "trace/matchmaking-pairing/readiness/contract.rs"]
mod trace_contract_matchmaking_pairing_readiness;
#[path = "trace/matchmaking-pairing/selection/contract.rs"]
mod trace_contract_matchmaking_pairing_selection;
#[path = "trace/matchmaking-pairing/windows/contract.rs"]
mod trace_contract_matchmaking_pairing_windows;
#[path = "trace/matchmaking-tickets/admission/contract.rs"]
mod trace_contract_matchmaking_tickets_admission;
#[path = "trace/matchmaking-tickets/browser-owner/contract.rs"]
mod trace_contract_matchmaking_tickets_browser_owner;
#[path = "trace/matchmaking-tickets/browser-response/contract.rs"]
mod trace_contract_matchmaking_tickets_browser_response;
#[path = "trace/matchmaking-tickets/browser-schema/contract.rs"]
mod trace_contract_matchmaking_tickets_browser_schema;
#[path = "trace/matchmaking-tickets/cancel/contract.rs"]
mod trace_contract_matchmaking_tickets_cancel;
#[path = "trace/matchmaking-tickets/current/contract.rs"]
mod trace_contract_matchmaking_tickets_current;
#[path = "trace/matchmaking-tickets/heartbeat/contract.rs"]
mod trace_contract_matchmaking_tickets_heartbeat;
#[path = "trace/matchmaking-tickets/rpc-boundary/contract.rs"]
mod trace_contract_matchmaking_tickets_rpc_boundary;
#[path = "trace/rating-application/atomic-application/contract.rs"]
mod trace_contract_rating_application_atomic_application;
#[path = "trace/rating-application/claims-fences/contract.rs"]
mod trace_contract_rating_application_claims_fences;
#[path = "trace/rating-application/pure-calculation/contract.rs"]
mod trace_contract_rating_application_pure_calculation;
#[path = "trace/rating-application/quarantine-recovery/contract.rs"]
mod trace_contract_rating_application_quarantine_recovery;
#[path = "trace/rating-application/terminal-job/contract.rs"]
mod trace_contract_rating_application_terminal_job;
#[path = "trace/rating-application/worker-process/contract.rs"]
mod trace_contract_rating_application_worker_process;
#[path = "trace/rating-application/worker-reconciliation/contract.rs"]
mod trace_contract_rating_application_worker_reconciliation;
#[path = "trace/durable-event-dispatch/feature.rs"]
mod trace_feature;
#[path = "trace/game-command-recovery/feature.rs"]
mod trace_feature_game_command_recovery;
#[path = "trace/game-play/feature.rs"]
mod trace_feature_game_play;
#[path = "trace/game-snapshots-access/feature.rs"]
mod trace_feature_game_snapshots_access;
#[path = "trace/matchmaking-pairing/feature.rs"]
mod trace_feature_matchmaking_pairing;
#[path = "trace/matchmaking-tickets/feature.rs"]
mod trace_feature_matchmaking_tickets;
#[path = "trace/rating-application/feature.rs"]
mod trace_feature_rating_application;
