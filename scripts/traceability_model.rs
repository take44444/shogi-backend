#![allow(dead_code, unused_imports, unused_macros)]

pub enum Relationship {
    Asserts,
    Exercises,
    Unreviewed,
}
pub struct Source {
    pub path: &'static str,
    pub needle: &'static str,
}
pub struct Evidence {
    pub feature: &'static str,
    pub scope: &'static str,
    pub contract: &'static str,
    pub relationship: Relationship,
    pub reason: &'static str,
}
pub struct Case {
    pub scope: &'static str,
    pub reason: &'static str,
    pub relationship: Relationship,
    pub also: &'static [Evidence],
}
pub struct Contract {
    pub statement: &'static str,
    pub source: Source,
    pub gap: Option<&'static str>,
}
pub struct Guarantee {
    pub id: &'static str,
    pub statement: &'static str,
    pub contracts: &'static [&'static str],
}
pub enum Surface {
    OpenApi {
        package: &'static str,
        example: &'static str,
        operation: &'static str,
    },
    Grpc {
        path: &'static str,
        service: &'static str,
        operation: &'static str,
    },
}
pub struct EntryPoint {
    pub name: &'static str,
    pub source: Source,
}
pub struct Feature {
    pub title: &'static str,
    pub source: Source,
    pub flow: &'static [&'static str],
    pub guarantees: &'static [Guarantee],
    pub limitations: &'static [&'static str],
    pub entrypoints: &'static [EntryPoint],
    pub surfaces: &'static [Surface],
}
pub struct Infrastructure {
    pub category: &'static str,
    pub reason: &'static str,
}
pub struct Helper {
    pub reason: &'static str,
}
pub enum ExampleKind {
    OpenApi,
    Program,
}
pub struct Example {
    pub kind: ExampleKind,
    pub reason: &'static str,
}
macro_rules! example {
    ($value:expr) => {
        const _: $crate::trace_support::Example = $value;
    };
}
macro_rules! trace_feature {
    ($value:expr) => {
        const _: $crate::trace_support::Feature = $value;
    };
}
macro_rules! contract {
    ($value:expr) => {
        const _: $crate::trace_support::Contract = $value;
    };
}
macro_rules! case {
    ($value:expr; $item:item) => {
        const _: $crate::trace_support::Case = $value;
        $item
    };
}
macro_rules! infrastructure {
    ($category:literal, $reason:literal; $item:item) => {
        const _: $crate::trace_support::Infrastructure = $crate::trace_support::Infrastructure {
            category: $category,
            reason: $reason,
        };
        $item
    };
}
macro_rules! helper {
    ($reason:literal; $item:item) => {
        const _: $crate::trace_support::Helper = $crate::trace_support::Helper { reason: $reason };
        $item
    };
}
pub(crate) use {case, contract, example, helper, infrastructure, trace_feature as feature};
