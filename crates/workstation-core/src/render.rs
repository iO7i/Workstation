use crate::*;
use serde::Serialize;
use std::fmt::Write;

/// Share mode is a new allowlisted object, not redaction of the original report.
#[derive(Debug, Serialize)]
pub struct ShareReport {
    pub schema_version: &'static str,
    pub product: &'static str,
    pub observed_at: String,
    pub coverage: Coverage,
    pub roots: Vec<ShareRoot>,
    pub selected_process_count: usize,
    pub process_coverage: Coverage,
    pub worktree_count: usize,
    pub finding_codes: Vec<String>,
    pub omitted: Vec<&'static str>,
}
#[derive(Debug, Serialize)]
pub struct ShareRoot {
    pub kind: RootKind,
    pub coverage: Coverage,
    pub logical_entry_bytes: Option<u64>,
    pub skipped_count: u64,
}
pub fn share_report(s: &Snapshot) -> ShareReport {
    const ALLOWED: &[&str] = &[
        "disk.low_available_space",
        "storage.incomplete_coverage",
        "storage.observed_growth",
    ];
    ShareReport {
        schema_version: SCHEMA_VERSION,
        product: "workstation-r0",
        observed_at: s.observed_at.clone(),
        coverage: s.coverage,
        roots: s
            .storage
            .iter()
            .map(|r| ShareRoot {
                kind: r.root.kind,
                coverage: r.coverage,
                logical_entry_bytes: r.logical_entry_bytes,
                skipped_count: r.skipped_links_or_placeholders,
            })
            .collect(),
        selected_process_count: s.processes.selected.len(),
        process_coverage: s.processes.coverage,
        worktree_count: s.workspaces.iter().map(|w| w.worktrees.len()).sum(),
        finding_codes: s
            .findings
            .iter()
            .filter(|f| ALLOWED.contains(&f.code.as_str()))
            .map(|f| f.code.clone())
            .collect(),
        omitted: vec![
            "paths",
            "project_and_root_names",
            "installation_and_process_ids",
            "command_lines",
            "raw_errors",
            "file_contents",
            "branch_names",
        ],
    }
}

pub fn terminal_safe(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            if c.is_control()
                || ('\u{202a}'..='\u{202e}').contains(&c)
                || ('\u{2066}'..='\u{2069}').contains(&c)
            {
                format!("\\u{{{:x}}}", u32::from(c))
                    .chars()
                    .collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect()
}
pub fn html_escape(s: &str) -> String {
    terminal_safe(s)
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
pub fn human_report(s: &Snapshot) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "WORKSTATION R0 | observed {} | {:?}",
        terminal_safe(&s.observed_at),
        s.coverage
    );
    let _ = writeln!(
        out,
        "Read-only snapshot. No cleanup or repairs were performed by this assessment.\n"
    );
    for d in &s.disks {
        let free = d
            .available_bytes
            .map(|n| format!("{:.2} GiB available", n as f64 / 1024_f64.powi(3)))
            .unwrap_or_else(|| "available space unknown".into());
        let _ = writeln!(
            out,
            "DISK {}: {} [{:?}]",
            terminal_safe(&d.target),
            free,
            d.coverage
        );
    }
    let _ = writeln!(
        out,
        "\nREGISTERED STORAGE (entry-logical bytes; not physical allocation)"
    );
    for r in &s.storage {
        let amount = r
            .logical_entry_bytes
            .map(|n| format!("{:.3} GiB", n as f64 / 1024_f64.powi(3)))
            .unwrap_or_else(|| "unknown".into());
        let _ = writeln!(
            out,
            "  {}: {} [{:?}] skipped={} errors={}\n    {}",
            terminal_safe(&r.root.id),
            amount,
            r.coverage,
            r.skipped_links_or_placeholders,
            r.io_errors,
            terminal_safe(&r.root.path.to_string_lossy())
        );
    }
    let _ = writeln!(
        out,
        "\nPROCESSES: {} selected [{:?}] — all protected, ownership unknown",
        s.processes.selected.len(),
        s.processes.coverage
    );
    for p in &s.processes.selected {
        let _ = writeln!(
            out,
            "  {} pid={} private_commit={:?} working_set={:?}",
            terminal_safe(&p.executable_name),
            p.pid,
            p.private_commit_bytes,
            p.working_set_bytes
        );
    }
    let _ = writeln!(
        out,
        "\nWORKSPACES (registration only; content not assessed)"
    );
    for w in &s.workspaces {
        let _ = writeln!(out, "  {} [{:?}]", terminal_safe(&w.project_id), w.coverage);
        for t in &w.worktrees {
            let _ = writeln!(out, "    {} [PROTECTED]", terminal_safe(&t.path));
        }
    }
    let _ = writeln!(out, "\nFINDINGS");
    for f in &s.findings {
        let _ = writeln!(
            out,
            "  {:?} {}: {}",
            f.severity,
            terminal_safe(&f.code),
            terminal_safe(&f.explanation)
        );
    }
    if s.findings.is_empty() {
        let _ = writeln!(
            out,
            "  No findings in inspected scope; this is not whole-machine certification."
        );
    }
    let _ = writeln!(
        out,
        "\nNOT INSPECTED: {}",
        terminal_safe(&s.not_inspected.join(", "))
    );
    let _ = writeln!(out, "Snapshot saved: {}", s.saved);
    out
}
pub fn html_report(s: &Snapshot) -> String {
    format!("<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'\"><title>Workstation R0 observation</title><style>body{{font:16px/1.6 system-ui,sans-serif;max-width:1100px;margin:3rem auto;padding:0 1.5rem;background:#fafafa;color:#202020}}pre{{white-space:pre-wrap;overflow-wrap:anywhere;border:1px solid #ddd;padding:1.5rem;background:white}}small{{color:#555}}</style><h1>Workstation R0</h1><p>Cached observation, not live monitoring. Local/private report; use <code>report --share</code> before sharing.</p><pre>{}</pre><small>No external assets. No scripts. No repair actions.</small></html>", html_escape(&human_report(s)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn html_escapes_active_markup() {
        assert_eq!(
            html_escape("<script>\"&'"),
            "&lt;script&gt;&quot;&amp;&#39;"
        );
    }
    #[test]
    fn terminal_controls_are_visible() {
        let s = terminal_safe("good\x1b[31m\u{202e}hidden\n");
        assert!(!s.contains('\x1b'));
        assert!(!s.contains('\u{202e}'));
        assert!(s.contains("\\u{1b}"));
    }
    #[test]
    fn unicode_survives() {
        assert_eq!(terminal_safe("مشروع 日本語"), "مشروع 日本語");
    }
}
