#![allow(dead_code, unused_imports, unused_macros)]

pub struct Source {
    pub path: &'static str,
    pub needle: &'static str,
}
pub struct External {
    pub id: &'static str,
    pub title: &'static str,
}
pub struct Component {
    pub description: &'static str,
    pub notes: &'static [&'static str],
    pub external: &'static [External],
    pub operations: &'static [Operation],
    pub entrypoints: &'static [EntryPoint],
}
pub struct EntryPoint {
    pub name: &'static str,
    pub source: Source,
}
pub enum Audience {
    Frontend,
    Internal,
    Operator,
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
    WebSocket {
        path: &'static str,
        command: &'static str,
        source: Source,
    },
}
pub struct Operation {
    pub contract: &'static str,
    pub audience: Audience,
    pub surface: Surface,
    pub reason: &'static str,
}
pub struct Feature {
    pub id: &'static str,
    pub title: &'static str,
    pub source: Source,
    pub flow: &'static [&'static str],
    pub guarantees: &'static [Guarantee],
    pub limitations: &'static [&'static str],
}
pub struct Guarantee {
    pub id: &'static str,
    pub statement: &'static str,
    pub roots: &'static [&'static str],
}
pub enum Interface {
    RustApi,
    Port,
    Http,
    Grpc,
    WebSocket,
    External,
}
pub struct Contract {
    pub id: &'static str,
    pub consumer: &'static str,
    pub provider: &'static str,
    pub interface: Interface,
    pub statement: &'static str,
    pub assumptions: &'static [&'static str],
    pub limitations: &'static [&'static str],
    pub source: Source,
    pub call: Option<Source>,
    pub implementation: Option<Source>,
    pub binding: Option<Source>,
    pub requires: &'static [Dependency],
    pub gap: Option<&'static str>,
}
pub struct Dependency {
    pub contract: &'static str,
    pub reason: &'static str,
}
pub enum Relationship {
    Asserts,
    Exercises,
    Unreviewed,
}
pub enum Observation {
    ProviderBoundary,
    CompleteInteraction,
}
pub struct Evidence {
    pub contract: &'static str,
    pub relationship: Relationship,
    pub scope: &'static str,
    pub reason: &'static str,
    pub observation: Observation,
    pub real: &'static [&'static str],
    pub fake: &'static [&'static str],
}
pub struct BoundaryCase {
    pub evidence: &'static [Evidence],
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
macro_rules! component {
    ($value:expr) => {
        const _: $crate::trace_support::Component = $value;
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
macro_rules! boundary_case {
    ($value:expr; $item:item) => {
        const _: $crate::trace_support::BoundaryCase = $value;
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
pub(crate) use {
    boundary_case, component, contract, example, helper, infrastructure, trace_feature as feature,
};
