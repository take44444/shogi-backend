use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The provisioned shogi_reader role can SELECT schema_version, games, journal and rating_states, cannot access rating/matchmaking internals and cannot write even when default read-only is disabled.",
    source: trace::Source {
        path: "scripts/init-postgres.sql",
        needle: "CREATE ROLE shogi_reader LOGIN PASSWORD 'shogi_reader';"
    },
    gap: Some(
        "These assertions use the test provisioned role. They do not prove arbitrary production credential ownership or permission policy."
    ),
});
