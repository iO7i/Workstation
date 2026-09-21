use crate::*;

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

pub fn summarize_coverage(items: impl IntoIterator<Item = Coverage>) -> Coverage {
    if items.into_iter().all(Coverage::is_complete) {
        Coverage::Complete
    } else {
        Coverage::Partial
    }
}

pub fn storage_delta(current: &StorageObservation, previous: &StorageObservation) -> Option<i64> {
    if current.root.path != previous.root.path
        || current.root.id != previous.root.id
        || !current.coverage.is_complete()
        || !previous.coverage.is_complete()
        || current.observed_at <= previous.observed_at
    {
        return None;
    }
    let delta =
        i128::from(current.logical_entry_bytes?) - i128::from(previous.logical_entry_bytes?);
    i64::try_from(delta).ok()
}

pub fn diagnose(snapshot: &Snapshot) -> Vec<Finding> {
    let mut out = Vec::new();
    for disk in &snapshot.disks {
        if disk.coverage.is_complete() {
            if let Some(free) = disk.available_bytes {
                if free < 25 * 1024_u64.pow(3) {
                    out.push(Finding {
                        code: "disk.low_available_space".into(),
                        severity: if free < 10 * 1024_u64.pow(3) { Severity::Critical } else { Severity::Warning },
                        target_id: Some(disk.target.clone()),
                        explanation: "Available space is below R0's default warning threshold (25 GiB). This is not proof of an agent fault.".into(),
                        next_action: "Review registered storage roots; do not delete unknown work.".into(),
                        repair_available: false,
                    });
                }
            }
        }
    }
    for s in &snapshot.storage {
        if !s.coverage.is_complete() {
            out.push(Finding { code: "storage.incomplete_coverage".into(), severity: Severity::Warning,
                target_id: Some(s.root.id.clone()), explanation: "The root was not completely measured. Available byte counts are not complete totals.".into(),
                next_action: "Inspect coverage notes; an explicit deep scan may help.".into(), repair_available: false });
        }
        if s.delta_since_previous_complete_bytes
            .is_some_and(|d| d > 1024_i64.pow(3))
        {
            out.push(Finding { code: "storage.observed_growth".into(), severity: Severity::Warning,
                target_id: Some(s.root.id.clone()), explanation: "Two complete observations show more than 1 GiB of entry-logical growth. Cause and reclaimability are unknown.".into(),
                next_action: "Review this root and its owning application; no automatic cleanup is offered.".into(), repair_available: false });
        }
    }
    out
}

pub fn exit_code(coverage: Coverage, findings: &[Finding], fail_on: Option<Severity>) -> u8 {
    let fail = findings.iter().any(|f| match fail_on {
        Some(Severity::Critical) => f.severity == Severity::Critical,
        Some(Severity::Warning) => f.severity != Severity::Info,
        Some(Severity::Info) => true,
        None => false,
    });
    if fail {
        2
    } else if !coverage.is_complete() {
        3
    } else {
        0
    }
}

/// Replay-only examples based on the user's reported incidents. Not live vendor diagnostics.
pub fn classify_incident(i: &IncidentInput) -> Finding {
    let (code, explanation, action) = match i.family {
        IncidentFamily::DockerSocket if !i.fresh_error => ("incident.stale_evidence", "An old socket error cannot establish a current failure.", "Collect a fresh current-startup error and local engine evidence."),
        IncidentFamily::DockerSocket => ("incident.docker_socket_reported", "The report describes a runtime socket failure, not corrupted Docker data.", "No R0 repair: preserve Docker data and validate stopped owners before a later reviewed recipe."),
        IncidentFamily::CodexAttachments if i.correct_registry_checked != Some(true) => ("incident.attachment_path_unverified", "A registry created at the wrong path does not prove the real attachment registry was repaired.", "Verify the active home and supported registry layout; preserve attachment files."),
        IncidentFamily::CodexAttachments => ("incident.attachment_runtime_test_needed", "A filesystem change is not proof that the large-paste workflow succeeds.", "Keep active sessions running; perform an explicit UI test when practical."),
        IncidentFamily::OldWorkspace => ("incident.workspace_protected", "Age, a missing PR, or a missing process is insufficient proof that a workspace is disposable.", "Preserve the workspace. Content and active-dependency checks are required before any later cleanup plan."),
    };
    Finding {
        code: code.into(),
        severity: Severity::Warning,
        target_id: None,
        explanation: explanation.into(),
        next_action: action.into(),
        repair_available: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ids_are_narrow() {
        for v in ["codex", "root-12", "a_b"] {
            assert!(valid_id(v));
        }
        for v in ["", "../x", "a b", "TOKEN=secret", "a\n", "Upper"] {
            assert!(!valid_id(v));
        }
        assert!(!valid_id(&"a".repeat(65)));
    }
    #[test]
    fn missing_coverage_never_green() {
        for c in [
            Coverage::Denied,
            Coverage::Unsupported,
            Coverage::TimedOut,
            Coverage::Partial,
        ] {
            assert_eq!(
                summarize_coverage([Coverage::Complete, c]),
                Coverage::Partial
            );
            assert_eq!(exit_code(c, &[], None), 3);
        }
    }
    #[test]
    fn warnings_do_not_implicitly_fail_complete_check() {
        let f = classify_incident(&IncidentInput {
            id: "test".into(),
            family: IncidentFamily::OldWorkspace,
            fresh_error: true,
            active_owner: None,
            correct_registry_checked: None,
            contents_inspected: false,
        });
        assert_eq!(
            exit_code(Coverage::Complete, std::slice::from_ref(&f), None),
            0
        );
        assert_eq!(
            exit_code(Coverage::Complete, &[f], Some(Severity::Warning)),
            2
        );
    }
    #[test]
    fn stale_docker_error_is_not_current_incident() {
        let f = classify_incident(&IncidentInput {
            id: "x".into(),
            family: IncidentFamily::DockerSocket,
            fresh_error: false,
            active_owner: Some(true),
            correct_registry_checked: None,
            contents_inspected: false,
        });
        assert_eq!(f.code, "incident.stale_evidence");
        assert!(!f.repair_available);
    }
    #[test]
    fn every_seed_incident_is_review_only() {
        for family in [
            IncidentFamily::DockerSocket,
            IncidentFamily::CodexAttachments,
            IncidentFamily::OldWorkspace,
        ] {
            let f = classify_incident(&IncidentInput {
                id: "x".into(),
                family,
                fresh_error: true,
                active_owner: Some(false),
                correct_registry_checked: Some(true),
                contents_inspected: true,
            });
            assert!(!f.repair_available);
        }
    }
    fn sample(size: u64, when: &str, coverage: Coverage) -> StorageObservation {
        StorageObservation {
            root: Root {
                id: "root".into(),
                path: std::path::PathBuf::from("D:/sample"),
                kind: RootKind::Other,
            },
            observed_at: when.into(),
            coverage,
            logical_entry_bytes: Some(size),
            allocated_bytes: None,
            files: 1,
            directories: 1,
            visited_entries: 1,
            skipped_links_or_placeholders: 0,
            io_errors: 0,
            notes: vec![],
            delta_since_previous_complete_bytes: None,
        }
    }
    #[test]
    fn partial_sizes_never_produce_growth_deltas() {
        let old = sample(100, "2026-09-18T10:00:00.000Z", Coverage::Complete);
        let new = sample(200, "2026-09-18T11:00:00.000Z", Coverage::Partial);
        assert_eq!(storage_delta(&new, &old), None);
    }
    #[test]
    fn complete_growth_and_clock_reversal_are_distinct() {
        let old = sample(100, "2026-09-18T10:00:00.000Z", Coverage::Complete);
        let new = sample(200, "2026-09-18T11:00:00.000Z", Coverage::Complete);
        assert_eq!(storage_delta(&new, &old), Some(100));
        assert_eq!(storage_delta(&old, &new), None);
    }
}
