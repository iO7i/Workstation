//! Historical association evidence. No result ever authorizes terminating a process.
use crate::{
    control::{self, Check},
    Coverage,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessKey {
    pub host: String,
    pub boot: String,
    pub pid: u32,
    pub created: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessFact {
    pub key: ProcessKey,
    pub parent: Option<ProcessKey>,
    pub executable_digest: Option<String>,
    pub private_bytes: Option<u64>,
    pub cpu_ticks: Option<u64>,
    pub io_bytes: Option<u64>,
    pub coverage: Coverage,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub process: ProcessKey,
    pub session_id: String,
    pub source: String,
    pub shared: bool,
    pub seen_at: i64,
    pub expires_at: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnershipInput {
    pub observed_at: i64,
    pub process_coverage: Coverage,
    pub processes: Vec<ProcessFact>,
    pub bindings: Vec<Binding>,
    pub ended_sessions: BTreeMap<String, i64>,
    pub previous: Vec<ProcessFact>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipRow {
    pub process: ProcessKey,
    pub sessions: Vec<String>,
    pub relation: String,
    pub retained_candidate: bool,
    pub protected: bool,
    pub reasons: Vec<String>,
}
pub fn correlate(input: &OwnershipInput) -> Check<Vec<OwnershipRow>> {
    if input.processes.len() > 4096
        || input.previous.len() > 4096
        || input.bindings.len() > 8192
        || input.ended_sessions.len() > 8192
    {
        return Err("OWNERSHIP_BUDGET");
    }
    control::timestamp(input.observed_at)?;
    let mut index = BTreeMap::new();
    for p in &input.processes {
        if p.key.created == 0 || p.key.host.is_empty() || p.key.boot.is_empty() {
            return Err("PROCESS_BIRTH_ID_REQUIRED");
        }
        if p.parent.as_ref().is_some_and(|a| {
            a.host != p.key.host || a.boot != p.key.boot || a.created > p.key.created || a == &p.key
        }) {
            return Err("INVALID_PARENT_BIRTH_IDENTITY");
        }
        if index.insert(p.key.clone(), p).is_some() {
            return Err("DUPLICATE_PROCESS_IDENTITY");
        }
    }
    let mut previous = BTreeMap::new();
    for p in &input.previous {
        if previous.insert(p.key.clone(), p).is_some() {
            return Err("DUPLICATE_PREVIOUS_PROCESS_IDENTITY");
        }
    }
    let mut bindings: BTreeMap<ProcessKey, Vec<&Binding>> = BTreeMap::new();
    for b in &input.bindings {
        control::id(&b.session_id)?;
        control::timestamp(b.seen_at)?;
        control::timestamp(b.expires_at)?;
        control::text(&b.source, 128)?;
        if b.expires_at < b.seen_at {
            return Err("BINDING_TIME_INVALID");
        }
        if b.seen_at <= input.observed_at && b.expires_at >= input.observed_at {
            bindings.entry(b.process.clone()).or_default().push(b);
        }
    }
    for (s, t) in &input.ended_sessions {
        control::id(s)?;
        control::timestamp(*t)?;
    }
    let mut rows = vec![];
    for p in &input.processes {
        let mut sessions = BTreeSet::new();
        let mut reasons = vec![];
        let mut shared = false;
        let mut cursor = Some(&p.key);
        let mut visited = BTreeSet::new();
        let mut supported = true;
        while let Some(key) = cursor {
            if !visited.insert(key.clone()) || visited.len() > 64 {
                supported = false;
                reasons.push("lineage_cycle_or_depth".into());
                break;
            }
            for b in bindings.get(key).into_iter().flatten() {
                control::id(&b.session_id)?;
                sessions.insert(b.session_id.clone());
                shared |= b.shared;
            }
            match index.get(key) {
                Some(parent) => {
                    if parent.coverage != Coverage::Complete {
                        supported = false;
                    }
                    cursor = parent.parent.as_ref();
                }
                None => {
                    supported = false;
                    break;
                }
            }
        }
        let explicitly_bound = bindings.get(&p.key).is_some_and(|v| {
            v.iter()
                .any(|b| b.source == "observed_owned_child_and_protocol_session")
        }) && p.executable_digest.is_some();
        let ended = !sessions.is_empty()
            && sessions.iter().all(|s| {
                input
                    .ended_sessions
                    .get(s)
                    .is_some_and(|t| *t <= input.observed_at - 60)
            });
        let retained = input.process_coverage == Coverage::Complete
            && p.coverage == Coverage::Complete
            && explicitly_bound
            && ended
            && !shared
            && previous.get(&p.key).is_some_and(|old| {
                old.executable_digest.is_some()
                    && old.executable_digest == p.executable_digest
                    && old.coverage == Coverage::Complete
            });
        if retained {
            reasons.push("same_process_observed_after_explicit_session_end_review_required".into());
        }
        if shared {
            reasons.push("shared_service_protected".into());
        }
        if sessions.is_empty() {
            reasons.push("no_supported_session_binding".into());
        }
        let relation = if sessions.is_empty() {
            "unknown"
        } else if shared || sessions.len() > 1 {
            "shared"
        } else if explicitly_bound {
            "explicit_binding"
        } else if supported {
            "observed_ancestry"
        } else {
            "partial_ancestry"
        };
        rows.push(OwnershipRow {
            process: p.key.clone(),
            sessions: sessions.into_iter().collect(),
            relation: relation.into(),
            retained_candidate: retained,
            protected: true,
            reasons,
        });
    }
    Ok(rows)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_snapshot_not_error() {
        let x = OwnershipInput {
            observed_at: 1,
            process_coverage: Coverage::Partial,
            processes: vec![],
            bindings: vec![],
            ended_sessions: BTreeMap::new(),
            previous: vec![],
        };
        assert!(correlate(&x).unwrap().is_empty());
    }
    #[test]
    fn process_key_distinguishes_pid_reuse() {
        assert_ne!(
            ProcessKey {
                host: "h".into(),
                boot: "b".into(),
                pid: 1,
                created: 1
            },
            ProcessKey {
                host: "h".into(),
                boot: "b".into(),
                pid: 1,
                created: 2
            }
        );
    }
}

/// Find a single registered executable in an observed configured-hook caller chain.
/// No strings from the hook payload can select a PID or claim exclusive ownership.
pub fn hook_ancestor(
    graph: &crate::host_graph::HostGraph,
    caller_pid: u32,
    integration_id: &str,
) -> Check<Option<ProcessKey>> {
    if graph.processes.len() > 4096 {
        return Err("OWNERSHIP_BUDGET");
    }
    let rows: Vec<_> = graph
        .processes
        .iter()
        .filter(|p| p.key.pid == caller_pid)
        .collect();
    if rows.len() != 1 {
        return Ok(None);
    }
    let index: BTreeMap<_, _> = graph.processes.iter().map(|p| (&p.key, p)).collect();
    let mut cursor = rows[0].parent.as_ref();
    let mut visited = BTreeSet::new();
    let mut matching = BTreeSet::new();
    while let Some(key) = cursor {
        if !visited.insert(key.clone()) || visited.len() > 64 {
            return Ok(None);
        }
        let Some(p) = index.get(key) else {
            return Ok(None);
        };
        if p.coverage != Coverage::Complete {
            return Ok(None);
        }
        if graph.executables.iter().any(|e| {
            &e.process == key && e.matching_integrations.iter().any(|x| x == integration_id)
        }) {
            matching.insert(key.clone());
        }
        cursor = p.parent.as_ref();
    }
    if matching.len() == 1 {
        Ok(matching.into_iter().next())
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod audit_binding_tests {
    use super::*;
    #[test]
    fn invalid_binding_time_is_not_silently_ignored() {
        let key = ProcessKey {
            host: "h".into(),
            boot: "b".into(),
            pid: 1,
            created: 1,
        };
        let x = OwnershipInput {
            observed_at: 100,
            process_coverage: Coverage::Partial,
            processes: vec![],
            bindings: vec![Binding {
                process: key,
                session_id: "s".into(),
                source: "reported".into(),
                shared: true,
                seen_at: 200,
                expires_at: 100,
            }],
            ended_sessions: BTreeMap::new(),
            previous: vec![],
        };
        assert!(correlate(&x).is_err());
    }
}
