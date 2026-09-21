//! Workspace relationships and lifecycle eligibility. Evidence is not delete authority.
use crate::{
    control::{self, Check, WorkState, WorkspaceStamp},
    Coverage,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceDetail {
    pub stamp: WorkspaceStamp,
    pub repository_identity: String,
    pub main_worktree: bool,
    pub bare: bool,
    pub locked: bool,
    pub changed_paths: Vec<String>,
    pub ignored_paths: Vec<String>,
    pub path_coverage: Coverage,
    pub branch_exists: Option<bool>,
    #[serde(default)]
    pub checkout_filters_present: Option<bool>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceActor {
    pub session_id: String,
    pub work_id: Option<String>,
    pub project_id: String,
    pub workspace_id: Option<String>,
    pub repository_identity: Option<String>,
    pub branch: Option<String>,
    pub paths: Vec<String>,
    pub paths_complete: bool,
    pub role: String,
    pub ended: bool,
    pub lease_until: Option<i64>,
    pub last_observed: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
    SameWorkItem,
    SameWorkspace,
    SameBranch,
    OverlappingPaths,
    UnknownScope,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConflict {
    pub first: String,
    pub second: String,
    pub kind: ConflictKind,
    pub risk: String,
    pub paths: Vec<String>,
    pub ownership_uncertain: bool,
    pub action: String,
}
pub fn relative_path(s: &str) -> Check<String> {
    control::text(s, 4096)?;
    let s = s.replace('\\', "/");
    if s.is_empty()
        || s.starts_with('/')
        || s.contains(':')
        || s.split('/').any(|x| x == ".." || x == "." || x.is_empty())
    {
        return Err("WORKSPACE_PATH_NOT_RELATIVE");
    }
    Ok(s)
}
fn key(s: &str) -> String {
    s.replace('\\', "/").to_ascii_lowercase()
}
#[cfg(test)]
fn overlap(a: &str, b: &str) -> bool {
    let a = key(a);
    let b = key(b);
    a == b || a.starts_with(&(b.clone() + "/")) || b.starts_with(&(a + "/"))
}
// O(path depth * log n) instead of comparing/allocating every path pair.
fn indexed_overlap(path: &str, others: &BTreeSet<String>) -> bool {
    if others.contains(path) {
        return true;
    }
    if path
        .match_indices('/')
        .any(|(i, _)| others.contains(&path[..i]))
    {
        return true;
    }
    let prefix = format!("{path}/");
    others
        .range(prefix.clone()..)
        .next()
        .is_some_and(|x| x.starts_with(&prefix))
}
/// A stale lease remains a possibly-active writer. Reviewers/observers are not auto-killed.
pub fn conflicts(actors: &[WorkspaceActor], now: i64) -> Check<Vec<WorkspaceConflict>> {
    control::timestamp(now)?;
    if actors.len() > 256 {
        return Err("ROSTER_LIMIT");
    }
    let mut ids = BTreeSet::new();
    let mut path_bytes = 0usize;
    let mut indexed = Vec::new();
    for a in actors {
        control::id(&a.session_id)?;
        control::id(&a.project_id)?;
        control::timestamp(a.last_observed)?;
        if let Some(t) = a.lease_until {
            control::timestamp(t)?;
        }
        if !["primary", "reviewer", "observer"].contains(&a.role.as_str()) {
            return Err("ACTOR_ROLE_INVALID");
        }
        if !ids.insert((&a.session_id, &a.work_id, &a.workspace_id)) {
            return Err("DUPLICATE_ACTOR");
        }
        if a.paths.len() > 512 {
            return Err("PATH_OVERLAP_BUDGET");
        }
        let mut normalized = BTreeSet::new();
        for p in &a.paths {
            relative_path(p)?;
            path_bytes = path_bytes
                .checked_add(p.len())
                .ok_or("PATH_OVERLAP_BUDGET")?;
            if path_bytes > 4 * 1024 * 1024 {
                return Err("PATH_OVERLAP_BUDGET");
            }
            normalized.insert(key(p));
        }
        indexed.push(normalized);
    }
    let mut result = Vec::new();
    for (n, a) in actors.iter().enumerate() {
        if a.ended || a.role == "observer" {
            continue;
        }
        for (j, b) in actors.iter().enumerate().skip(n + 1) {
            if b.ended || b.role == "observer" || a.session_id == b.session_id {
                continue;
            }
            let same_repo =
                a.repository_identity.is_some() && a.repository_identity == b.repository_identity;
            let same_workspace = a.project_id == b.project_id
                && a.workspace_id.is_some()
                && a.workspace_id == b.workspace_id;
            let same_work = a.project_id == b.project_id
                && a.work_id.is_some()
                && a.work_id == b.work_id
                && a.role == "primary"
                && b.role == "primary";
            let same_branch = same_repo && a.branch.is_some() && a.branch == b.branch;
            let mut paths = BTreeSet::new();
            if same_repo || same_workspace {
                for p in &a.paths {
                    if indexed_overlap(&key(p), &indexed[j]) {
                        paths.insert(p.clone());
                        if paths.len() == 32 {
                            break;
                        }
                    }
                }
            }
            let (kind, risk) = if same_work {
                (ConflictKind::SameWorkItem, "high")
            } else if same_workspace {
                (ConflictKind::SameWorkspace, "high")
            } else if !paths.is_empty() {
                (ConflictKind::OverlappingPaths, "medium")
            } else if same_branch {
                (ConflictKind::SameBranch, "medium")
            } else if same_repo && (!a.paths_complete || !b.paths_complete) {
                (ConflictKind::UnknownScope, "unknown")
            } else {
                continue;
            };
            let uncertain = [a, b].iter().any(|x| {
                x.last_observed > now
                    || now - x.last_observed > 300
                    || x.lease_until.is_none_or(|t| t < now)
            });
            result.push(WorkspaceConflict {
                first: a.session_id.clone(),
                second: b.session_id.clone(),
                kind,
                risk: risk.into(),
                paths: paths.into_iter().take(32).collect(),
                ownership_uncertain: uncertain,
                action: "coordinate_or_use_separate_workspace_no_termination".into(),
            });
            if result.len() > 1024 {
                return Err("COLLISION_RESULT_LIMIT");
            }
        }
    }
    Ok(result)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CleanupContext {
    pub detail: WorkspaceDetail,
    pub work_states: BTreeMap<String, WorkState>,
    pub unresolved_sessions: Vec<String>,
    pub protection_reasons: Vec<String>,
    pub observations_complete: bool,
    /// Explicit human coordination declaration. Not a scan-based proof of no writers.
    pub quiescence_acknowledged: bool,
    pub preserve_head_ref: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupEligibility {
    pub disposition: String,
    pub blockers: Vec<String>,
    pub required_preservation: Vec<String>,
    pub automatic_delete_authorized: bool,
}
pub fn cleanup_eligibility(c: &CleanupContext, now: i64) -> Check<CleanupEligibility> {
    control::timestamp(now)?;
    let s = &c.detail.stamp;
    control::timestamp(s.observed_at)?;
    let mut b = BTreeSet::new();
    if c.detail.main_worktree || c.detail.bare {
        b.insert("main_or_bare_checkout".into());
    }
    if c.detail.locked {
        b.insert("git_worktree_locked".into());
    }
    if !c.observations_complete
        || c.detail.path_coverage != Coverage::Complete
        || s.coverage != Coverage::Complete
    {
        b.insert("incomplete_evidence".into());
    }
    if now < s.observed_at || now - s.observed_at > 30 {
        b.insert("stale_workspace_evidence".into());
    }
    if s.dirty != Some(false) {
        b.insert("dirty_or_unknown_tracked_state".into());
    }
    if s.untracked != Some(0) {
        b.insert("untracked_or_unknown_content".into());
    }
    if s.ignored != Some(0) || !c.detail.ignored_paths.is_empty() {
        b.insert("ignored_or_unknown_local_data".into());
    }
    if !c.detail.changed_paths.is_empty() {
        b.insert("changed_paths".into());
    }
    if !s.blockers.is_empty() {
        b.insert("git_operation_or_inspection_blocker".into());
    }
    if !c.unresolved_sessions.is_empty() {
        b.insert("possibly_active_session".into());
    }
    if c.work_states
        .values()
        .any(|x| !matches!(x, WorkState::Done | WorkState::Cancelled))
    {
        b.insert("unfinished_work_item".into());
    }
    if !c.protection_reasons.is_empty() {
        b.insert("explicit_protection".into());
    }
    if !c.quiescence_acknowledged {
        b.insert("external_writers_not_coordinated".into());
    }
    if !c.preserve_head_ref {
        b.insert("head_preservation_required".into());
    }
    let ok = b.is_empty();
    Ok(CleanupEligibility {
        disposition: if ok {
            "eligible_for_exact_reviewed_plan"
        } else {
            "protected"
        }
        .into(),
        blockers: b.into_iter().collect(),
        required_preservation: vec![
            "Retain exact HEAD under a Workstation-owned Git ref; never delete the branch.".into(),
            "Capture metadata receipt before non-force Git operation; no broad prune.".into(),
        ],
        automatic_delete_authorized: false,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn actor(s: &str) -> WorkspaceActor {
        WorkspaceActor {
            session_id: s.into(),
            work_id: None,
            project_id: "p".into(),
            workspace_id: None,
            repository_identity: Some("repo".into()),
            branch: Some("refs/heads/main".into()),
            paths: vec!["src/auth.rs".into()],
            paths_complete: true,
            role: "primary".into(),
            ended: false,
            lease_until: Some(99),
            last_observed: 1,
        }
    }
    #[test]
    fn branch_overlap_is_detected() {
        let a = actor("a");
        let b = actor("b");
        assert!(!conflicts(&[a, b], 2).unwrap().is_empty());
    }
    #[test]
    fn expiry_never_removes_actor() {
        let a = actor("a");
        let b = actor("b");
        assert!(conflicts(&[a, b], 200).unwrap()[0].ownership_uncertain);
    }
    #[test]
    fn observers_not_writers() {
        let a = actor("a");
        let mut b = actor("b");
        b.role = "observer".into();
        assert!(conflicts(&[a, b], 2).unwrap().is_empty());
    }
    #[test]
    fn parent_path_overlap() {
        assert!(overlap("src/Auth", "src/auth/token.rs"));
        assert!(!overlap("src/a", "src/abc"));
    }
    #[test]
    fn reject_path_traversal() {
        for p in ["../a", "C:/file", "/root", "a/./b"] {
            assert!(relative_path(p).is_err());
        }
    }
}

#[cfg(test)]
mod audit_collision_tests {
    use super::*;
    #[test]
    fn prefix_index_handles_lexical_interlopers() {
        let set = ["a", "a-b"].into_iter().map(str::to_owned).collect();
        assert!(indexed_overlap("a/x", &set));
        assert!(!indexed_overlap("abc", &set));
    }
    #[test]
    fn prefix_index_matches_pairwise_reference() {
        let paths = [
            "a",
            "a/x",
            "a-b",
            "a.b",
            "src/auth",
            "src/auth/token.rs",
            "src/abc",
            "z",
        ];
        for mask in 0..(1usize << paths.len()) {
            let set: BTreeSet<String> = paths
                .iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, x)| x.to_string())
                .collect();
            for a in &paths {
                assert_eq!(indexed_overlap(a, &set), set.iter().any(|b| overlap(a, b)));
            }
        }
    }
    #[test]
    fn negative_workspace_time_is_rejected() {
        let bad = WorkspaceActor {
            session_id: "a".into(),
            work_id: None,
            project_id: "p".into(),
            workspace_id: None,
            repository_identity: None,
            branch: None,
            paths: vec![],
            paths_complete: false,
            role: "primary".into(),
            ended: false,
            lease_until: None,
            last_observed: i64::MIN,
        };
        assert!(conflicts(&[bad], 1).is_err());
    }
}
