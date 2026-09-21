//! Versioned integration intent. Registration and documentation are not certification.
use crate::{
    control::{self, Check},
    Coverage,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Adapter {
    Codex,
    Claude,
    CursorAcp,
    GrokAcp,
    GeminiAcp,
    Copilot,
    OpenCode,
    Cline,
    Roo,
    Windsurf,
    Doppler,
    OnePassword,
    Docker,
    Worktrunk,
    Entire,
}
impl Adapter {
    pub fn id(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::CursorAcp => "cursor_acp",
            Self::GrokAcp => "grok_acp",
            Self::GeminiAcp => "gemini_acp",
            Self::Copilot => "copilot",
            Self::OpenCode => "opencode",
            Self::Cline => "cline",
            Self::Roo => "roo",
            Self::Windsurf => "windsurf",
            Self::Doppler => "doppler",
            Self::OnePassword => "onepassword",
            Self::Docker => "docker",
            Self::Worktrunk => "worktrunk",
            Self::Entire => "entire",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Integration {
    pub id: String,
    pub project_id: String,
    pub environment: String,
    pub adapter: Adapter,
    pub executable: String,
    pub executable_sha256: String,
    /// Exact observed --version text. A new binary/version needs new registration.
    pub version_text: String,
    /// Auth/config is passed by explicitly named inherited variables only.
    pub inherit_env: Vec<String>,
    /// Overrides may contain paths/configuration, never credentials.
    pub configuration: BTreeMap<String, String>,
    /// Provider auth variables map to Atlas resource IDs, resolved at execution only.
    pub authentication: BTreeMap<String, String>,
    #[serde(default)]
    pub protocol_revision: Option<u32>,
    /// Explicit noninteractive auth method; must also be advertised by ACP.
    #[serde(default)]
    pub auth_method: Option<String>,
    pub observed_at: i64,
}
impl Integration {
    pub fn validate(&self) -> Check<()> {
        if self.protocol_revision.is_some_and(|v| v == 0 || v > 32) {
            return Err("PROTOCOL_REVISION_INVALID");
        }
        if let Some(method) = &self.auth_method {
            control::id(method)?;
        }
        control::id(&self.id)?;
        control::id(&self.project_id)?;
        control::id(&self.environment)?;
        absolute_path(&self.executable)?;
        digest(&self.executable_sha256)?;
        control::text(&self.version_text, 512)?;
        control::timestamp(self.observed_at)?;
        if self.version_text.trim().is_empty() {
            return Err("EXACT_VERSION_REQUIRED");
        }
        if self.inherit_env.len() > 20
            || self.configuration.len() > 16
            || self.authentication.len() > 4
        {
            return Err("INTEGRATION_LIMIT");
        }
        for key in &self.inherit_env {
            if ![
                "PATH",
                "USERPROFILE",
                "HOME",
                "APPDATA",
                "LOCALAPPDATA",
                "CODEX_HOME",
                "CLAUDE_CONFIG_DIR",
                "DOCKER_CONFIG",
                "OP_CONFIG_DIR",
                "DOPPLER_CONFIG_DIR",
                "COPILOT_HOME",
                "GEMINI_CLI_HOME",
                "XDG_DATA_HOME",
                "XDG_CONFIG_HOME",
            ]
            .contains(&key.as_str())
            {
                return Err("INHERITED_ENV_NOT_ALLOWLISTED");
            }
        }
        for (key, value) in &self.configuration {
            if ![
                "CODEX_HOME",
                "CLAUDE_CONFIG_DIR",
                "DOCKER_CONFIG",
                "OP_CONFIG_DIR",
                "DOPPLER_CONFIG_DIR",
                "APPDATA",
                "LOCALAPPDATA",
                "COPILOT_HOME",
                "GEMINI_CLI_HOME",
                "XDG_DATA_HOME",
                "XDG_CONFIG_HOME",
            ]
            .contains(&key.as_str())
            {
                return Err("CONFIG_ENV_NOT_ALLOWLISTED");
            }
            absolute_path(value)?;
        }
        if self.adapter == Adapter::Copilot && self.authentication.len() > 1 {
            return Err("ONE_COPILOT_AUTH_SOURCE_REQUIRED");
        }
        for (key, resource) in &self.authentication {
            if !auth_variable(self.adapter, key) {
                return Err("PROVIDER_AUTH_ENV_MISMATCH");
            }
            control::id(resource)?;
        }
        Ok(())
    }
}
pub fn auth_variable(adapter: Adapter, key: &str) -> bool {
    match adapter {
        Adapter::Doppler => key == "DOPPLER_TOKEN",
        Adapter::OnePassword => key == "OP_SERVICE_ACCOUNT_TOKEN",
        Adapter::Claude => key == "ANTHROPIC_API_KEY",
        Adapter::Codex => key == "OPENAI_API_KEY",
        Adapter::CursorAcp => ["CURSOR_API_KEY", "CURSOR_AUTH_TOKEN"].contains(&key),
        Adapter::GrokAcp => key == "XAI_API_KEY",
        Adapter::GeminiAcp => key == "GEMINI_API_KEY",
        Adapter::Copilot => ["GH_TOKEN", "GITHUB_TOKEN"].contains(&key),
        Adapter::Cline => key == "CLINE_API_KEY",
        _ => false,
    }
}
pub fn digest(s: &str) -> Check<()> {
    if s.len() == 64 && s.bytes().all(|x| x.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err("SHA256_REQUIRED")
    }
}
pub fn absolute_path(s: &str) -> Check<()> {
    control::text(s, 32700)?;
    let windows =
        s.as_bytes().get(1) == Some(&b':') && matches!(s.as_bytes().get(2), Some(b'\\' | b'/'));
    if !(s.starts_with('/') || windows) || s.starts_with("//") || s.starts_with("\\\\") {
        return Err("ABSOLUTE_LOCAL_PATH_REQUIRED");
    }
    if s.split(['/', '\\']).any(|x| x == ".." || x == ".") {
        return Err("DOT_PATH_REJECTED");
    }
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterCapabilities {
    pub adapter: Adapter,
    pub implementation_revision: String,
    pub operations: Vec<String>,
    pub unsupported: Vec<String>,
    pub certification: String,
    pub tested_versions: Vec<String>,
    pub coverage: Coverage,
    pub sources: Vec<String>,
}
pub fn capabilities(a: Adapter) -> AdapterCapabilities {
    let (ops, unsupported, sources): (&[&str], &[&str], &[&str]) = match a {
        Adapter::Codex => (
            &[
                "version_probe",
                "thread_list",
                "quota_read",
                "new_turn",
                "native_resume",
            ],
            &["private_desktop_db_repair_certification"],
            &["https://developers.openai.com/codex/app-server/"],
        ),
        Adapter::Claude => (
            &[
                "version_probe",
                "json_print_continuation",
                "native_resume",
                "lifecycle_import",
                "statusline_import",
                "configured_statusline_hook",
            ],
            &["exact_independent_subscription_poll"],
            &[
                "https://code.claude.com/docs/en/cli-reference",
                "https://code.claude.com/docs/en/hooks",
            ],
        ),
        Adapter::CursorAcp => (
            &[
                "version_probe",
                "acp_new",
                "acp_load_if_advertised",
                "acp_prompt",
            ],
            &["individual_quota_api"],
            &["https://cursor.com/docs/cli/acp"],
        ),
        Adapter::GrokAcp => (
            &[
                "version_probe",
                "acp_new",
                "acp_load_if_advertised",
                "acp_prompt",
            ],
            &["subscription_quota_api"],
            &["https://docs.x.ai/build/cli/headless-scripting"],
        ),
        Adapter::GeminiAcp => (
            &[
                "version_probe",
                "acp_initialize",
                "acp_new",
                "acp_resume_if_advertised",
                "acp_list_if_advertised",
                "acp_prompt",
                "lifecycle_hook",
                "session_metrics_import",
            ],
            &["read_only_plan_guarantee", "consumer_quota_api"],
            &[
                "https://geminicli.com/docs/cli/acp-mode/",
                "https://geminicli.com/docs/hooks/",
                "https://geminicli.com/docs/cli/headless/",
            ],
        ),
        Adapter::OpenCode => (
            &[
                "version_probe",
                "acp_initialize",
                "acp_new",
                "acp_resume_if_advertised",
                "acp_list_if_advertised",
                "acp_prompt",
            ],
            &["consumer_quota_api", "unregistered_native_session_resume"],
            &["https://opencode.ai/docs/acp/"],
        ),
        Adapter::Cline => (
            &[
                "version_probe",
                "acp_initialize",
                "acp_new",
                "acp_resume_if_advertised",
                "acp_list_if_advertised",
                "acp_prompt",
            ],
            &["consumer_quota_api", "automatic_doctor_fix"],
            &["https://github.com/cline/cline/blob/main/docs/usage/acp.mdx"],
        ),
        Adapter::Copilot => (
            &[
                "version_probe",
                "pinned_stdio_sdk_wire_v3",
                "account_quota_read",
                "model_prices_read",
                "session_list",
                "new_turn",
                "native_resume",
                "lifecycle_event_import",
            ],
            &[
                "protocol_versions_other_than_3",
                "telemetry_identity_verification",
                "all_vscode_extension_state",
            ],
            &[
                "https://github.com/github/copilot-sdk",
                "https://docs.github.com/en/copilot/how-tos/copilot-sdk/features/usage-and-billing",
            ],
        ),
        Adapter::Worktrunk => (
            &[
                "version_probe",
                "schema2_local_list",
                "exact_approved_worktree_create_no_hooks",
            ],
            &[
                "merge_rebase",
                "branch_deleting_remove",
                "project_hook_automation",
            ],
            &[
                "https://worktrunk.dev/list/",
                "https://worktrunk.dev/switch/",
            ],
        ),
        Adapter::Doppler => (
            &["version_probe", "one_secret_resolution"],
            &["secret_mutations", "account_creation"],
            &["https://docs.doppler.com/docs/accessing-secrets"],
        ),
        Adapter::OnePassword => (
            &["version_probe", "one_secret_resolution"],
            &["vault_administration"],
            &["https://developer.1password.com/docs/cli/reference/commands/read/"],
        ),
        Adapter::Docker => (
            &[
                "version_probe",
                "local_engine_probe",
                "scoped_offline_runtime_quarantine",
            ],
            &["global_wsl_shutdown", "data_reset"],
            &["https://docs.docker.com/engine/manage-resources/contexts/"],
        ),
        _ => (
            &["version_probe", "explicit_installation_inventory"],
            &["native_continuation", "live_quota"],
            &[],
        ),
    };
    AdapterCapabilities {
        adapter: a,
        implementation_revision: "workstation.adapters.v4".into(),
        operations: ops.iter().map(|x| (*x).into()).collect(),
        unsupported: unsupported.iter().map(|x| (*x).into()).collect(),
        certification: "declaration_not_runtime_certification".into(),
        tested_versions: vec![],
        coverage: Coverage::Partial,
        sources: sources.iter().map(|x| (*x).into()).collect(),
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilePin {
    pub path: String,
    pub sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovedTask {
    pub id: String,
    pub project_id: String,
    pub environment: String,
    pub executable: String,
    pub executable_sha256: String,
    pub cwd: String,
    pub directory_identity: String,
    pub args: Vec<String>,
    pub script_pins: Vec<FilePin>,
    pub secret_bindings: BTreeMap<String, String>,
    pub timeout_seconds: u32,
    pub output_policy: String,
}
impl ApprovedTask {
    pub fn validate(&self) -> Check<()> {
        control::id(&self.id)?;
        control::id(&self.project_id)?;
        control::id(&self.environment)?;
        absolute_path(&self.executable)?;
        absolute_path(&self.cwd)?;
        digest(&self.executable_sha256)?;
        control::text(&self.directory_identity, 256)?;
        if self.args.len() > 64
            || self.script_pins.len() > 32
            || self.secret_bindings.len() > 32
            || !(1..=3600).contains(&self.timeout_seconds)
        {
            return Err("TASK_BUDGET");
        }
        if self.output_policy != "discard" {
            return Err("TASK_OUTPUT_MUST_BE_DISCARDED");
        }
        let mut length = 0;
        for a in &self.args {
            control::text(a, 2048)?;
            length += a.len();
        }
        if length > 12000 {
            return Err("TASK_ARGUMENT_BUDGET");
        }
        for p in &self.script_pins {
            absolute_path(&p.path)?;
            digest(&p.sha256)?;
        }
        for (k, v) in &self.secret_bindings {
            safe_secret_env(k)?;
            control::id(v)?;
        }
        // Inline interpreter payloads defeat script pinning. A real executable + fixed args is allowed.
        let exe = self
            .executable
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or("")
            .to_lowercase();
        if [
            "cmd.exe",
            "powershell.exe",
            "pwsh.exe",
            "bash.exe",
            "sh.exe",
            "cmd",
            "powershell",
            "pwsh",
            "bash",
            "sh",
        ]
        .contains(&exe.as_str())
        {
            return Err("SHELL_TASK_NOT_SUPPORTED_USE_PINNED_EXECUTABLE");
        }
        if ["node.exe", "node", "python.exe", "python", "python3"].contains(&exe.as_str())
            && self.script_pins.is_empty()
        {
            return Err("INTERPRETER_SCRIPT_PIN_REQUIRED");
        }
        if self.args.iter().any(|a| {
            [
                "-e",
                "--eval",
                "-c",
                "--command",
                "-Command",
                "-EncodedCommand",
            ]
            .contains(&a.as_str())
        }) {
            return Err("INLINE_CODE_ARGUMENT_REJECTED");
        }
        Ok(())
    }
}
pub fn safe_secret_env(k: &str) -> Check<()> {
    if k.is_empty()
        || k.len() > 128
        || !k
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err("ENVIRONMENT_KEY_INVALID");
    }
    if [
        "PATH",
        "PATHEXT",
        "HOME",
        "USERPROFILE",
        "TEMP",
        "TMP",
        "SYSTEMROOT",
        "WINDIR",
        "COMSPEC",
        "BASH_ENV",
        "ENV",
        "NODE_OPTIONS",
        "NODE_PATH",
        "PYTHONPATH",
        "PYTHONHOME",
        "PYTHONSTARTUP",
        "RUBYOPT",
        "PERL5OPT",
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
    ]
    .contains(&k)
        || k.starts_with("DYLD_")
        || k.starts_with("GIT_")
    {
        return Err("BEHAVIOR_CHANGING_SECRET_ENV_REJECTED");
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_certification_in_capability() {
        let caps = capabilities(Adapter::Codex);
        assert!(caps.tested_versions.is_empty());
        assert_eq!(caps.certification, "declaration_not_runtime_certification");
    }
    #[test]
    fn dynamic_loader_env_not_secret() {
        assert!(safe_secret_env("NODE_OPTIONS").is_err());
    }
    #[test]
    fn database_env_ok() {
        assert!(safe_secret_env("DATABASE_URL").is_ok());
    }
    #[test]
    fn path_not_relative() {
        assert!(absolute_path("relative.exe").is_err());
    }
    #[test]
    fn refs_not_paths() {
        assert!(absolute_path("https://example.com").is_err());
    }
}
