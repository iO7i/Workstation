//! Pure contracts and conservative diagnoses. No filesystem, process, or network effects.
#![forbid(unsafe_code)]
pub mod model;
pub mod policy;
pub mod render;
pub use model::*;

pub mod continuity;
pub mod control;
pub mod discovery;
pub mod economics;
pub mod plans;

pub mod protocol;

pub mod diagnostics;

pub mod integrations;

pub mod effects;

pub mod ownership;

pub mod lifecycle;

pub mod usage_import;

pub mod health_rules;
pub mod operations;
pub mod workspace_policy;

pub mod host_graph;

pub mod wire;

pub mod telemetry;

pub mod durable;
