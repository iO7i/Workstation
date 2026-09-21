//! Platform effects: observation plus explicitly planned, scoped, journaled integrations.
//! No automatic effects or read-only MCP mutation path.
#![deny(unsafe_op_in_unsafe_fn)]
pub mod filesystem;
pub mod git;
pub mod paths;
pub mod runner;
pub mod storage;
pub mod windows;

use std::fmt;
#[derive(Debug)]
pub struct Error {
    pub code: &'static str,
}
impl Error {
    pub fn new(code: &'static str) -> Self {
        Self { code }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

pub fn now() -> String {
    let fmt = time::format_description::parse(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z",
    )
    .expect("static timestamp format");
    time::OffsetDateTime::now_utc()
        .format(&fmt)
        .expect("UTC formatting")
}
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub mod control_store;
pub mod control_workspace;
pub mod vault;

pub mod external;

pub mod integration_store;

pub mod rpc;
pub mod tasks;

pub mod adapters;

pub mod http_provider;

pub mod repairs;

pub mod engine;

pub mod project_tools;

pub mod profile_health;

pub mod operations_store;

pub mod process_graph;
pub mod workspace_lifecycle;

pub mod health_scan;

pub mod secret_lifecycle;

pub mod journal_archive;

pub mod operation_runtime;

pub mod copilot;

pub mod acp_driver;

pub mod specialists;

pub mod telemetry_store;

pub mod capability_context;

pub mod durable_compat;
pub mod durable_runtime;
pub mod durable_store;
pub mod durable_verify;
