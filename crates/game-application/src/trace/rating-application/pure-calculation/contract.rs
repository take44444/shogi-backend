use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The pure calculator operates on validated RatingState values, computes one rounded K32 delta from effective starts, applies its negation to white and checks rating/count overflow.",
    source: trace::Source {
        path: "crates/rating-domain/src/lib.rs",
        needle: "pub fn calculate("
    },
    gap: Some(
        "These finite cases are not a mathematical proof or an exhaustive floating-point rounding portability guarantee."
    ),
});
