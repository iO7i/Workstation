//! One owner supervisor, durable evidence, and explicit recovery. Never replay an effect.
use crate::{
    control_store::{epoch, sha},
    storage::Store,
    Error, Result,
};
use serde_json::{json, Value};
use std::{
    cell::RefCell,
    path::Path,
    time::{Duration, Instant},
};
use workstation_core::{durable::*, integrations::Adapter};

#[derive(Clone, Copy)]
enum Stage {
    Handshake,
    Session,
    Turn,
}
struct Monitor {
    store: Store,
    id: String,
    owner: ProcessIdentity,
    start: Instant,
    stage_at: Instant,
    stage: Stage,
    last_progress: Instant,
    last_flush: Instant,
    last_heartbeat: Instant,
    policy: DeadlinePolicy,
    record: RunRecord,
    cancel_at: Option<Instant>,
    stop_code: Option<&'static str>,
    cancel_sent: bool,
    pending_terminal: Option<(String, String, ProviderState)>,
    progress_keys: std::collections::BTreeSet<String>,
}
thread_local! {static MONITOR:RefCell<Option<Monitor>>=const{RefCell::new(None)};}
pub struct MonitorScope;
impl Drop for MonitorScope {
    fn drop(&mut self) {
        MONITOR.with(|m| {
            m.borrow_mut().take();
        });
    }
}
pub fn install(home: &Path, id: &str) -> Result<MonitorScope> {
    let store = Store::open(home)?;
    let record = store.durable_run(id)?;
    let owner = self_identity(&store.config.installation_id)?;
    if record.supervisor.as_ref() != Some(&owner) {
        return Err(Error::new("DURABLE_OWNER_MISMATCH"));
    }
    MONITOR.with(|m| {
        let mut current = m.borrow_mut();
        if current.is_some() {
            return Err(Error::new("DURABLE_NESTED_MONITOR"));
        }
        *current = Some(Monitor {
            store,
            id: id.into(),
            owner,
            start: Instant::now(),
            stage_at: Instant::now(),
            stage: Stage::Handshake,
            last_progress: Instant::now(),
            last_flush: Instant::now(),
            last_heartbeat: Instant::now(),
            policy: record.deadlines.clone(),
            record,
            cancel_at: None,
            stop_code: None,
            cancel_sent: false,
            pending_terminal: None,
            progress_keys: std::collections::BTreeSet::new(),
        });
        Ok(MonitorScope)
    })
}
fn with<T>(f: impl FnOnce(&mut Monitor) -> Result<T>) -> Result<Option<T>> {
    MONITOR.with(|cell| {
        let mut b = cell.borrow_mut();
        b.as_mut().map(f).transpose()
    })
}
impl Monitor {
    fn event(&mut self, e: RunEvent) -> Result<()> {
        self.record = self.store.durable_event(&self.id, e)?;
        Ok(())
    }
    fn bind_session(&mut self, s: &str) -> Result<()> {
        if self.record.session_id.as_deref() != Some(s) {
            self.event(RunEvent::SessionBound {
                session_id: s.into(),
            })?;
        }
        self.stage = Stage::Turn;
        self.stage_at = Instant::now();
        self.last_progress = Instant::now();
        Ok(())
    }
    fn terminal(&mut self, session: &str, turn: Option<&str>, status: ProviderState) -> Result<()> {
        if self.record.session_id.as_deref() != Some(session) {
            return Ok(());
        }
        if let Some(t) = turn {
            if self.record.turn_id.as_deref() != Some(t) {
                return Ok(());
            }
        }
        if self.record.provider != status {
            self.event(RunEvent::ProviderTerminal { status })?;
        }
        Ok(())
    }
}
pub fn child_started(child: &std::process::Child) -> Result<()> {
    with(|m| {
        let (pid, birth) = crate::process_graph::child_identity(child)?;
        m.event(RunEvent::ChildStarted {
            identity: ProcessIdentity {
                pid,
                birth: birth.to_string(),
                host: m.store.config.installation_id.clone(),
            },
        })
    })?;
    Ok(())
}
pub fn session_bound(s: &str) -> Result<()> {
    with(|m| m.bind_session(s))?;
    Ok(())
}
pub fn request_started(method: &str) -> Result<()> {
    with(|m| {
        if method == "initialize" {
            // Provider startup and local plan revalidation are outside the wire handshake.
            // Start the handshake budget when the first handshake request is submitted.
            m.stage = Stage::Handshake;
            m.stage_at = Instant::now();
        } else if matches!(
            method,
            "thread/start" | "thread/resume" | "session/new" | "session/load" | "authenticate"
        ) {
            m.stage = Stage::Session;
            m.stage_at = Instant::now();
        } else if matches!(method, "turn/start" | "session/prompt") {
            m.stage = Stage::Turn;
            m.stage_at = Instant::now();
            m.last_progress = Instant::now();
        }
        Ok(())
    })?;
    Ok(())
}
pub fn response_received(method: &str, v: &Value) -> Result<()> {
    with(|m| {
        if matches!(method, "thread/start" | "thread/resume") {
            if let Some(s) = v.pointer("/thread/id").and_then(Value::as_str) {
                m.bind_session(s)?;
            }
        }
        if method == "session/new" {
            if let Some(s) = v.get("sessionId").and_then(Value::as_str) {
                m.bind_session(s)?;
            }
        }
        if method == "turn/start" {
            if let Some(t) = v.pointer("/turn/id").and_then(Value::as_str) {
                if m.record.turn_id.as_deref() != Some(t) {
                    m.event(RunEvent::TurnBound { turn_id: t.into() })?;
                }
                if let Some((s, pt, p)) = m.pending_terminal.take() {
                    m.terminal(&s, Some(&pt), p)?;
                }
            }
        }
        if method == "session/prompt" {
            let status = match v.get("stopReason").and_then(Value::as_str) {
                Some("end_turn") => Some(ProviderState::Completed),
                Some("cancelled") => Some(ProviderState::Interrupted),
                _ => None,
            };
            if let (Some(status), Some(s)) = (status, m.record.session_id.clone()) {
                m.terminal(&s, None, status)?;
            }
        }
        Ok(())
    })?;
    Ok(())
}
/// Project a protocol message into fixed kinds and opaque IDs. Never retain text/diff/tool parameters.
pub fn observe(v: &Value) -> Result<()> {
    with(|m| {
        let method = v.get("method").and_then(Value::as_str).unwrap_or("");
        if method == "turn/completed" {
            if let (Some(s), Some(t), Some(status)) = (
                v.pointer("/params/threadId").and_then(Value::as_str),
                v.pointer("/params/turn/id").and_then(Value::as_str),
                v.pointer("/params/turn/status").and_then(Value::as_str),
            ) {
                let p = match status {
                    "completed" => ProviderState::Completed,
                    "failed" => ProviderState::Failed,
                    "interrupted" => ProviderState::Interrupted,
                    _ => return Ok(()),
                };
                opaque(s).map_err(Error::new)?;
                opaque(t).map_err(Error::new)?;
                if m.record.turn_id.is_none() {
                    m.pending_terminal = Some((s.into(), t.into(), p));
                } else {
                    m.terminal(s, Some(t), p)?;
                }
            }
            return Ok(());
        }
        let codex = matches!(
            method,
            "item/started" | "item/completed" | "turn/diff/updated" | "turn/plan/updated"
        );
        let acp = method == "session/update"
            && matches!(
                v.pointer("/params/update/sessionUpdate")
                    .and_then(Value::as_str),
                Some("tool_call" | "tool_call_update" | "plan")
            );
        let scoped = if codex {
            v.pointer("/params/threadId").and_then(Value::as_str) == m.record.session_id.as_deref()
                && m.record.session_id.is_some()
        } else if acp {
            v.pointer("/params/sessionId").and_then(Value::as_str) == m.record.session_id.as_deref()
                && m.record.session_id.is_some()
        } else {
            false
        };
        let event_id = v
            .pointer("/params/item/id")
            .or_else(|| v.pointer("/params/update/toolCallId"))
            .and_then(Value::as_str);
        let progress_key = event_id
            .filter(|s| s.len() <= 256 && !s.chars().any(char::is_control))
            .map(|id| {
                format!(
                    "{method}:{id}:{}",
                    v.pointer("/params/update/status")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                )
            });
        if scoped
            && progress_key
                .is_some_and(|key| m.progress_keys.len() < 4096 && m.progress_keys.insert(key))
        {
            m.last_progress = Instant::now();
            if m.last_flush.elapsed() >= Duration::from_secs(1) {
                m.event(RunEvent::Progress)?;
                m.last_flush = Instant::now();
            }
        }
        Ok(())
    })?;
    Ok(())
}
/// Called by the owned transport at most every 25ms. Timeout/cancel can only stop this owner job.
pub fn tick() -> Result<Option<Value>> {
    Ok(with(|m|{
        if m.last_heartbeat.elapsed()>=Duration::from_millis(250){
            m.record=m.store.durable_heartbeat(&m.id,&m.owner)?;m.last_heartbeat=Instant::now();
        }
        if m.cancel_at.is_none(){
            let stop=if m.record.cancellation_requested{Some("DURABLE_CANCEL_REQUESTED")}
                else if m.start.elapsed()>=Duration::from_secs(m.policy.absolute_seconds.into()){Some("DURABLE_ABSOLUTE_TIMEOUT")}
                else if matches!(m.stage,Stage::Handshake)&&m.stage_at.elapsed()>=Duration::from_secs(m.policy.handshake_seconds.into()){Some("DURABLE_HANDSHAKE_TIMEOUT")}
                else if matches!(m.stage,Stage::Session)&&m.stage_at.elapsed()>=Duration::from_secs(m.policy.session_seconds.into()){Some("DURABLE_SESSION_TIMEOUT")}
                else if matches!(m.stage,Stage::Turn)&&!provider_terminal(m.record.provider)&&m.last_progress.elapsed()>=Duration::from_secs(m.policy.idle_seconds.into()){Some("DURABLE_IDLE_TIMEOUT")}else{None};
            if let Some(code)=stop{
                let mut effective_code=code;
                let deadline=match code{"DURABLE_HANDSHAKE_TIMEOUT"=>Some(DeadlineKind::Handshake),"DURABLE_SESSION_TIMEOUT"=>Some(DeadlineKind::Session),"DURABLE_IDLE_TIMEOUT"=>Some(DeadlineKind::Idle),"DURABLE_ABSOLUTE_TIMEOUT"=>Some(DeadlineKind::Absolute),_=>None};
                if let Some(deadline)=deadline{
                    m.event(RunEvent::DeadlineExceeded{deadline})?;
                    // durable_event resolves the cancellation/deadline race while holding
                    // the SQLite write transaction. A previously persisted cancellation wins.
                    if m.record.cancellation_requested && m.record.deadline_expired.is_none(){
                        effective_code="DURABLE_CANCEL_REQUESTED";
                    }
                }
                m.stop_code=Some(effective_code);m.cancel_at=Some(Instant::now());
            }
        }
        if let Some(at)=m.cancel_at{
            if at.elapsed()>=Duration::from_secs(m.policy.cancellation_grace_seconds.into()){
                return Err(Error::new(m.stop_code.unwrap_or("DURABLE_CANCEL_REQUESTED")));
            }
            if !m.cancel_sent{
                m.cancel_sent=true;
                if m.record.adapter=="codex"{if let (Some(s),Some(t))=(&m.record.session_id,&m.record.turn_id){return Ok(Some(json!({"jsonrpc":"2.0","id":"workstation-cancel","method":"turn/interrupt","params":{"threadId":s,"turnId":t}})));}}
                if m.record.adapter.ends_with("acp")||matches!(m.record.adapter.as_str(),"opencode"|"cline"){
                    if let Some(s)=&m.record.session_id{return Ok(Some(json!({"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":s}})));}
                }
            }
        }Ok(None)
    })?.flatten())
}
pub fn active() -> bool {
    MONITOR.with(|m| m.borrow().is_some())
}
pub fn finish(store: &mut Store, id: &str, result: &Result<Value>) -> Result<()> {
    let r = store.durable_run(id)?;
    if let Ok(v) = result {
        let ended = v
            .pointer("/continuation/completed")
            .and_then(Value::as_bool)
            == Some(true)
            || v.pointer("/continuation/turn_status")
                .and_then(Value::as_str)
                == Some("completed");
        if ended && !provider_terminal(r.provider) {
            store.durable_event(
                id,
                RunEvent::ProviderTerminal {
                    status: ProviderState::Completed,
                },
            )?;
        }
    }
    let (mut state, error) = match result {
        Ok(_) => (TransportState::Exited, None),
        Err(e) if e.code == "DURABLE_CANCEL_REQUESTED" => {
            (TransportState::Cancelled, Some(e.code.into()))
        }
        Err(e) if e.code.contains("TIMEOUT") => (TransportState::TimedOut, Some(e.code.into())),
        Err(e) => (TransportState::Lost, Some(e.code.into())),
    };
    if r.deadline_expired.is_some() {
        state = TransportState::TimedOut;
    } else if r.cancellation_requested && r.provider != ProviderState::Completed {
        state = TransportState::Cancelled;
    }
    store.durable_event(
        id,
        RunEvent::TransportEnded {
            state,
            error_code: error,
        },
    )?;
    Ok(())
}
#[cfg(windows)]
pub fn process_birth(pid: u32) -> Result<Option<String>> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GetLastError, ERROR_INVALID_PARAMETER, FILETIME},
        System::Threading::*,
    };
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return if GetLastError() == ERROR_INVALID_PARAMETER {
                Ok(None)
            } else {
                Err(Error::new("PROCESS_IDENTITY_UNAVAILABLE"))
            };
        }
        let mut c: FILETIME = std::mem::zeroed();
        let (mut e, mut k, mut u) = (c, c, c);
        let ok = GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u);
        let mut exit = 0;
        let active = GetExitCodeProcess(h, &mut exit) != 0 && exit == 259;
        CloseHandle(h);
        if ok == 0 {
            return Err(Error::new("PROCESS_IDENTITY_UNAVAILABLE"));
        }
        if !active {
            return Ok(None);
        }
        Ok(Some(
            (u64::from(c.dwLowDateTime) | (u64::from(c.dwHighDateTime) << 32)).to_string(),
        ))
    }
}
#[cfg(not(windows))]
pub fn process_birth(_pid: u32) -> Result<Option<String>> {
    Err(Error::new("NATIVE_PROCESS_IDENTITY_REQUIRED"))
}
pub fn self_identity(host: &str) -> Result<ProcessIdentity> {
    let pid = std::process::id();
    let birth = process_birth(pid)?.ok_or(Error::new("SUPERVISOR_IDENTITY_UNAVAILABLE"))?;
    Ok(ProcessIdentity {
        pid,
        birth,
        host: host.into(),
    })
}
pub fn supervisor_alive(store: &Store, r: &RunRecord) -> Result<bool> {
    let Some(p) = &r.supervisor else {
        return Ok(false);
    };
    if p.host != store.config.installation_id {
        return Err(Error::new("RUN_HOST_IDENTITY_CHANGED"));
    }
    Ok(process_birth(p.pid)?.as_deref() == Some(&p.birth))
}
/// Local evidence only. No login, protocol request, prompt, or automatic assignment release.
pub fn reconcile_local(store: &mut Store, id: &str, expected: u64) -> Result<Value> {
    let mut r = store.durable_run(id)?;
    if r.revision != expected {
        return Err(Error::new("DURABLE_REVISION_CONFLICT"));
    }
    let alive = supervisor_alive(store, &r)?;
    if !r.owner_released && r.execution_id.is_some() && !alive {
        r = store.durable_event(
            id,
            RunEvent::Recovered {
                supervisor_gone: true,
            },
        )?;
    }
    if alive && !r.owner_released {
        return Ok(
            json!({"run":r,"supervisor_alive":true,"replay":false,"provider_queried":false}),
        );
    }
    let workspace = match crate::engine::capture(store, &r.project_id, &r.workspace_id) {
        Ok(live) => {
            let cp = store.checkpoint(&r.checkpoint_id)?;
            if live.directory_identity != cp.workspace.directory_identity
                || live.path != cp.workspace.path
            {
                WorkspaceState::Unexpected
            } else if live.coverage != workstation_core::Coverage::Complete {
                WorkspaceState::Unavailable
            } else if live.status_digest != cp.workspace.status_digest
                || live.head != cp.workspace.head
            {
                WorkspaceState::Changed
            } else {
                WorkspaceState::Unchanged
            }
        }
        Err(_) => WorkspaceState::Unavailable,
    };
    if r.workspace != workspace {
        r = store.durable_event(id, RunEvent::WorkspaceObserved { state: workspace })?;
    }
    Ok(
        json!({"state":r.state(),"run":r,"provider_queried":false,"automatic_replay":false,"ownership_released":false,"workspace_content_verified":false}),
    )
}
pub fn recovery_scan(store: &mut Store, project: &str) -> Result<Value> {
    let runs = store.durable_runs(project)?;
    let mut found = vec![];
    for r in runs
        .into_iter()
        .filter(|r| r.execution_id.is_some() && !r.owner_released)
    {
        match supervisor_alive(store, &r) {
            Ok(false) => {
                let updated = store.durable_event(
                    &r.id,
                    RunEvent::Recovered {
                        supervisor_gone: true,
                    },
                )?;
                found.push(
                    json!({"id":r.id,"state":updated.state(),"supervisor":"gone","replayed":false}),
                );
            }
            Ok(true) => found.push(json!({"id":r.id,"supervisor":"alive","changed":false})),
            Err(_) => found
                .push(json!({"id":r.id,"supervisor":"unknown","changed":false,"protected":true})),
        }
    }
    Ok(json!({"runs":found,"automatic_replay":false,"assignments_released":false}))
}
pub fn reconcile_approval(store: &Store, id: &str) -> Result<String> {
    let r = store.durable_run(id)?;
    let i = store.integration(&r.integration_id)?;
    Ok(sha(serde_json::to_vec(&json!({"run":r,"integration":i,"operation":"read_exact_codex_turn","expires_bucket":epoch()/300})).map_err(|_|Error::new("ENCODING_FAILED"))?.as_slice()))
}
pub fn reconcile_provider(store: &mut Store, id: &str, approval: &str) -> Result<Value> {
    if reconcile_approval(store, id)? != approval {
        return Err(Error::new("RECONCILE_APPROVAL_CHANGED"));
    }
    let r = store.durable_run(id)?;
    if supervisor_alive(store, &r)? && !r.owner_released {
        return Err(Error::new("RUN_SUPERVISOR_STILL_ACTIVE"));
    }
    let i = store.integration(&r.integration_id)?;
    if i.project_id != r.project_id
        || i.environment != r.environment
        || i.executable_sha256 != r.executable_sha256
        || i.version_text != r.declared_version
    {
        return Err(Error::new("RECONCILE_INTEGRATION_CHANGED"));
    }
    if i.adapter != Adapter::Codex {
        return Err(Error::new("ADAPTER_RECONCILE_UNSUPPORTED_NO_REPLAY"));
    }
    let (s, t) = (
        r.session_id
            .as_deref()
            .ok_or(Error::new("RECONCILE_SESSION_UNKNOWN"))?,
        r.turn_id
            .as_deref()
            .ok_or(Error::new("RECONCILE_TURN_UNKNOWN"))?,
    );
    let timeout = r.deadlines.reconciliation_seconds;
    let mut rpc = crate::adapters::codex(store, &i, &store.home.to_string_lossy(), timeout)?;
    // Read-only persisted evidence. NEVER start/resume a thread or a turn here.
    let raw = rpc.request("thread/read", json!({"threadId":s,"includeTurns":true}))?;
    let status = codex_turn_status(&raw, s, t).map_err(Error::new)?;
    if let Some(status) = status {
        if r.provider != status {
            store.durable_event(id, RunEvent::Reconciled { status })?;
        }
    }
    let updated = store.durable_run(id)?;
    Ok(
        json!({"run":updated,"exact_turn_status":status,"provider_queried":true,"session_resumed":false,"prompt_submitted":false,"raw_content_retained":false,"task_verified":false}),
    )
}
