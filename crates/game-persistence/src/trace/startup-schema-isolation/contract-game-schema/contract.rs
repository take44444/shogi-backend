use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The writer initializer creates an empty schema, checks the required fingerprint and version seven under an advisory transaction lock, and grants four public tables to an existing reader role.",
    source: trace::Source {
        path: "crates/game-persistence/src/schema.rs",
        needle: "pub(crate) async fn initialize(db: &DatabaseConnection) -> Result<(), SchemaError> {"
    },
    gap: Some(
        "Preserved tested schema/data snapshots do not imply absence of all SQL effects. Writer validation creates a temporary expected schema and can grant reader SELECT; the reader has a stricter column policy than the writer subset check."
    ),
});
