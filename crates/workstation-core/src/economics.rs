//! Deterministic advisory calculations. Quotas, context and dollars never share a total.
use crate::control::{self, Check, Evidence, EvidenceKind};
use crate::Coverage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Meter {
    Context,
    Subscription,
    Dollars,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotaSample {
    pub id: String,
    pub provider: String,
    pub account_alias: String,
    pub bucket: String,
    pub meter: Meter,
    pub unit: String,
    pub window_id: String,
    pub used: f64,
    pub limit: Option<f64>,
    pub reset_at: Option<i64>,
    pub observed_at: i64,
    pub precision: String,
    pub evidence: Evidence,
}
impl QuotaSample {
    pub fn validate(&self) -> Check<()> {
        for s in [&self.id, &self.provider, &self.account_alias, &self.bucket] {
            control::id(s)?;
        }
        control::text(&self.window_id, 128)?;
        control::timestamp(self.observed_at)?;
        control::evidence(&self.evidence)?;
        if !self.used.is_finite() || self.used < 0.0 || self.used > 1e15 {
            return Err("INVALID_USAGE");
        }
        if self
            .limit
            .is_some_and(|x| !x.is_finite() || x <= 0.0 || x > 1e15)
        {
            return Err("INVALID_QUOTA");
        }
        if !["exact", "estimated", "provider_reported"].contains(&self.precision.as_str()) {
            return Err("PRECISION_REQUIRED");
        }
        let valid = match self.meter {
            Meter::Dollars => self.unit == "usd",
            Meter::Context => ["tokens", "percent"].contains(&self.unit.as_str()),
            Meter::Subscription => {
                ["percent", "credits", "requests", "tokens"].contains(&self.unit.as_str())
            }
        };
        if !valid {
            return Err("INCOMPATIBLE_METER_UNIT");
        }
        if let Some(t) = self.reset_at {
            control::timestamp(t)?;
        }
        Ok(())
    }
    fn same_stream(&self, other: &Self) -> bool {
        self.provider == other.provider
            && self.account_alias == other.account_alias
            && self.bucket == other.bucket
            && self.meter == other.meter
            && self.unit == other.unit
            && self.window_id == other.window_id
            && self.reset_at == other.reset_at
            && self.limit == other.limit
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Horizon {
    pub label: String,
    pub observed_span_seconds: i64,
    pub rate_per_second: Option<f64>,
    pub exhaustion_after_seconds: Option<f64>,
    pub exhaustion_before_reset: Option<bool>,
    pub capacity_at_current_pace_seconds: Option<f64>,
    pub headroom_to_reset_seconds: Option<f64>,
    pub shortfall_seconds: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runway {
    pub provider: String,
    pub bucket: String,
    pub meter: Meter,
    pub unit: String,
    pub window_id: String,
    pub remaining: Option<f64>,
    pub used: f64,
    pub reset_at: Option<i64>,
    pub observed_at: i64,
    pub as_of: i64,
    pub coverage: Coverage,
    pub estimate_label: String,
    pub confidence: String,
    pub horizons: Vec<Horizon>,
    pub notes: Vec<String>,
}
fn finite_ratio(numerator: f64, denominator: f64) -> Option<f64> {
    if !numerator.is_finite() || !denominator.is_finite() || denominator <= 0.0 {
        return None;
    }
    let ratio = numerator / denominator;
    ratio.is_finite().then_some(ratio)
}
fn horizon(
    label: &str,
    rate: Option<f64>,
    remaining: Option<f64>,
    reset_in: Option<i64>,
    span: i64,
) -> Horizon {
    let eta = match (rate, remaining) {
        (_, Some(r)) if r <= 0.0 => Some(0.0),
        (Some(b), Some(r)) if b > 0.0 => finite_ratio(r, b),
        _ => None,
    };
    let rate = rate.filter(|v| v.is_finite());
    Horizon {
        label: label.into(),
        observed_span_seconds: span,
        rate_per_second: rate,
        exhaustion_after_seconds: if eta.zip(reset_in).is_some_and(|(e, r)| e >= r as f64) {
            None
        } else {
            eta
        },
        capacity_at_current_pace_seconds: eta,
        headroom_to_reset_seconds: eta.zip(reset_in).map(|(e, r)| (e - r as f64).max(0.0)),
        exhaustion_before_reset: eta.zip(reset_in).map(|(e, r)| e < (r.max(0) as f64)),
        shortfall_seconds: eta.zip(reset_in).map(|(e, r)| (r as f64 - e).max(0.0)),
    }
}
/// Last uninterrupted stream/window only; gaps and counter decreases start a new segment.
/// A stale sample or unknown limit yields no precise quota forecast.
pub fn forecast(samples: &[QuotaSample], now: i64) -> Check<Runway> {
    control::timestamp(now)?;
    if samples.is_empty() || samples.len() > 10000 {
        return Err("SAMPLE_COUNT");
    }
    for s in samples {
        s.validate()?;
    }
    let mut rows: Vec<&QuotaSample> = samples.iter().filter(|s| s.observed_at <= now).collect();
    rows.sort_by_key(|s| s.observed_at);
    let latest = *rows.last().ok_or("NO_NONFUTURE_SAMPLE")?;
    if rows.iter().any(|s| {
        s.provider != latest.provider
            || s.account_alias != latest.account_alias
            || s.bucket != latest.bucket
            || s.meter != latest.meter
            || s.unit != latest.unit
    }) {
        return Err("MIXED_STREAMS");
    }
    let mut result=Runway{provider:latest.provider.clone(),bucket:latest.bucket.clone(),meter:latest.meter,unit:latest.unit.clone(),window_id:latest.window_id.clone(),
        remaining:latest.limit.map(|l|(l-latest.used).max(0.0)),used:latest.used,reset_at:latest.reset_at,observed_at:latest.observed_at,as_of:now,
        coverage:Coverage::Partial,estimate_label:"conditional_estimate_not_entitlement".into(),confidence:"insufficient".into(),horizons:vec![],
        notes:vec!["Estimates assume comparable recent activity; quotas from different providers are not added. Capacity-at-pace is conditional; no exhaustion ETA is predicted after a reset.".into()]};
    if latest.meter != Meter::Subscription {
        result
            .notes
            .push("Context compaction and dollar billing are not subscription runway.".into());
        return Ok(result);
    }
    if latest.reset_at.is_some_and(|r| r <= now) {
        result.notes.push(
            "Reset passed; fetch a new observation. Previous allowance is not treated as current."
                .into(),
        );
        return Ok(result);
    }
    if now - latest.observed_at > 900 {
        result
            .notes
            .push("Latest sample is older than 15 minutes; forecast withheld.".into());
        return Ok(result);
    }
    if latest.limit.is_none() {
        result.notes.push("Quota capacity is unknown; burn can be observed but remaining time cannot be inferred.".into());
        return Ok(result);
    }
    if rows
        .windows(2)
        .any(|w| w[0].observed_at == w[1].observed_at)
    {
        return Err("DUPLICATE_SAMPLE_TIME");
    }
    let mut segment = vec![latest];
    for older in rows.iter().rev().skip(1) {
        let newer = *segment.last().ok_or("EMPTY_SEGMENT")?;
        if !older.same_stream(latest)
            || older.used > newer.used
            || newer.observed_at - older.observed_at > 21600
        {
            result.notes.push("A reset, changed capacity, correction or observation gap ends the forecast baseline.".into());
            break;
        }
        segment.push(*older);
    }
    segment.reverse();
    let span = latest.observed_at - segment[0].observed_at;
    if segment.len() < 2 || span < 60 {
        result
            .notes
            .push("At least two comparable samples spanning one minute are required.".into());
        return Ok(result);
    }
    let reset_in = latest.reset_at.map(|r| r - now);
    for (label, seconds) in [("10m", 600_i64), ("1h", 3600), ("6h", 21600)] {
        let start = latest.observed_at - seconds;
        let mut burned = 0.0;
        let mut elapsed = 0_i64;
        for w in segment.windows(2) {
            let overlap = (w[1].observed_at - w[0].observed_at.max(start)).max(0);
            let dt = w[1].observed_at - w[0].observed_at;
            if overlap > 0 {
                burned += (w[1].used - w[0].used) * (overlap as f64) / (dt as f64);
                elapsed += overlap;
            }
        }
        let rate = if elapsed >= 60 {
            Some(burned / elapsed as f64)
        } else {
            None
        };
        result
            .horizons
            .push(horizon(label, rate, result.remaining, reset_in, elapsed));
    }
    let baseline = (latest.used - segment[0].used) / (span as f64);
    let mut weights = 0.0;
    let mut weighted = 0.0;
    for w in segment.windows(2) {
        let dt = (w[1].observed_at - w[0].observed_at) as f64;
        let weight = dt * 2_f64.powf(-((latest.observed_at - w[1].observed_at) as f64) / 3600.0);
        weighted += ((w[1].used - w[0].used) / dt) * weight;
        weights += weight;
    }
    let ewma = weighted / weights;
    result.horizons.push(horizon(
        "observed_window_baseline",
        Some(baseline),
        result.remaining,
        reset_in,
        span,
    ));
    result.horizons.push(horizon(
        "ewma_1h_half_life",
        Some(ewma),
        result.remaining,
        reset_in,
        span,
    ));
    result.horizons.push(horizon(
        "blended",
        Some(0.65 * ewma + 0.35 * baseline),
        result.remaining,
        reset_in,
        span,
    ));
    result.confidence = if segment.len() >= 6 && span >= 3600 {
        "moderate"
    } else {
        "low"
    }
    .into();
    if baseline == 0.0 {
        result
            .notes
            .push("No observed burn; no finite exhaustion time is justified.".into());
    }
    if latest.observed_at < now {
        result.notes.push(
            "Remaining is the last observed allowance, not an unobserved consumption adjustment."
                .into(),
        );
    }
    Ok(result)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cycle {
    pub provider: String,
    #[serde(default)]
    pub account_alias: Option<String>,
    pub bucket: String,
    pub window_id: String,
    pub utilization: f64,
    pub complete: bool,
    pub evidence_ref: String,
}
pub fn plan_fit(cycles: &[Cycle]) -> Check<serde_json::Value> {
    if cycles.len() > 128 {
        return Err("CYCLE_LIMIT");
    }
    let mut values = Vec::new();
    let mut ids = std::collections::BTreeSet::new();
    let mut stream = None;
    let mut unknown_account = false;
    for c in cycles {
        if !c.utilization.is_finite() || c.utilization < 0.0 || c.evidence_ref.is_empty() {
            return Err("CYCLE_INVALID");
        }
        control::id(&c.provider)?;
        control::id(&c.bucket)?;
        control::text(&c.window_id, 128)?;
        control::text(&c.evidence_ref, 512)?;
        if c.window_id.is_empty() {
            return Err("CYCLE_IDENTITY_REQUIRED");
        }
        if !c.complete {
            continue;
        }
        let Some(account) = &c.account_alias else {
            unknown_account = true;
            continue;
        };
        control::id(account)?;
        let key = (&c.provider, account, &c.bucket);
        if stream.as_ref().is_some_and(|s| *s != key) {
            return Err("MIXED_CYCLES");
        }
        stream = Some(key);
        if !ids.insert(&c.window_id) {
            return Err("DUPLICATE_CYCLE");
        }
        values.push(c.utilization);
    }
    if unknown_account {
        return Ok(
            serde_json::json!({"status":"unknown","reason":"explicit_account_identity_required"}),
        );
    }
    if values.len() < 3 {
        return Ok(
            serde_json::json!({"status":"unknown","reason":"three_complete_cycles_required"}),
        );
    }
    values.sort_by(f64::total_cmp);
    let n = values.len();
    let median = if n % 2 == 1 {
        values[n / 2]
    } else {
        values[n / 2 - 1] / 2.0 + values[n / 2] / 2.0
    };
    let exceeded = values.iter().filter(|&&x| x >= 1.0).count();
    let status = if exceeded >= 2 {
        "repeatedly_exceeded"
    } else if median < 0.3 {
        "underutilized"
    } else if median < 0.8 {
        "comfortable"
    } else {
        "tight"
    };
    Ok(
        serde_json::json!({"status":status,"median_utilization":median,"complete_cycles":n,"advisory_only":true,"does_not_recommend_extra_spending":true}),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelMetric {
    pub model: String,
    pub benchmark: String,
    pub revision: String,
    pub quality: f64,
    pub task_cost_usd: f64,
    pub latency_ms: Option<f64>,
    pub available: bool,
    pub license_state: String,
    pub source: String,
}
pub fn pareto(models: &[ModelMetric]) -> Check<serde_json::Value> {
    if models.len() > 500 {
        return Err("MODEL_LIMIT");
    }
    let mut names = std::collections::BTreeSet::new();
    for m in models {
        control::text(&m.model, 128)?;
        control::text(&m.source, 512)?;
        control::text(&m.benchmark, 256)?;
        control::text(&m.revision, 256)?;
        if m.model.is_empty()
            || m.source.is_empty()
            || m.benchmark.is_empty()
            || m.revision.is_empty()
            || !names.insert(&m.model)
        {
            return Err("MODEL_IDENTITY_INVALID");
        }
        if !m.quality.is_finite()
            || !m.task_cost_usd.is_finite()
            || m.task_cost_usd < 0.0
            || m.latency_ms.is_some_and(|x| !x.is_finite() || x < 0.0)
        {
            return Err("MODEL_METRIC_INVALID");
        }
        if m.license_state != "permitted_local_use" {
            return Err("BENCHMARK_LICENSE_NOT_PERMITTED");
        }
    }
    let eligible: Vec<_> = models.iter().filter(|m| m.available).collect();
    if let Some(first) = eligible.first() {
        if eligible
            .iter()
            .any(|m| m.benchmark != first.benchmark || m.revision != first.revision)
        {
            return Err("INCOMPARABLE_BENCHMARKS");
        }
    }
    let frontier: Vec<_> = eligible
        .iter()
        .filter(|a| {
            !eligible.iter().any(|b| {
                b.quality >= a.quality
                    && b.task_cost_usd <= a.task_cost_usd
                    && (b.quality > a.quality || b.task_cost_usd < a.task_cost_usd)
            })
        })
        .map(|m| m.model.clone())
        .collect();
    Ok(
        serde_json::json!({"frontier":frontier,"value_density_heuristic":eligible.iter().map(|m|serde_json::json!({"model":m.model,"index_points_per_usd":if m.task_cost_usd>0.0&&m.quality>=0.0{finite_ratio(m.quality,m.task_cost_usd)}else{None}})).collect::<Vec<_>>(),"dimensions":["higher_quality","lower_task_cost_usd"],"note":"Benchmark quality is an index, not ratio-scale intelligence. No subscription quota conversion performed."}),
    )
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioBucket {
    pub id: String,
    pub unit: String,
    pub remaining: f64,
    pub current_rate_per_second: f64,
    pub proposed_rate_multiplier: f64,
    pub resets_in_seconds: Option<f64>,
}
pub fn counterfactual(
    buckets: &[ScenarioBucket],
    assumptions: &[String],
) -> Check<serde_json::Value> {
    if buckets.is_empty() || buckets.len() > 20 || assumptions.is_empty() {
        return Err("SCENARIO_ASSUMPTIONS_REQUIRED");
    }
    control::lines(assumptions)?;
    let mut seen = std::collections::BTreeSet::new();
    let mut rows = vec![];
    for b in buckets {
        control::id(&b.id)?;
        control::text(&b.unit, 32)?;
        if b.unit.is_empty() {
            return Err("SCENARIO_UNIT_REQUIRED");
        }
        if !seen.insert(&b.id)
            || [
                b.remaining,
                b.current_rate_per_second,
                b.proposed_rate_multiplier,
            ]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0)
            || b.proposed_rate_multiplier > 10.0
            || b.resets_in_seconds
                .is_some_and(|x| !x.is_finite() || x < 0.0)
        {
            return Err("SCENARIO_INVALID");
        }
        let eta = |rate: f64| {
            if b.remaining == 0.0 {
                Some(0.0)
            } else if rate > 0.0 {
                finite_ratio(b.remaining, rate)
            } else {
                None
            }
        };
        let proposed_rate = b.current_rate_per_second * b.proposed_rate_multiplier;
        if !proposed_rate.is_finite()
            || (b.current_rate_per_second > 0.0
                && b.proposed_rate_multiplier > 0.0
                && proposed_rate == 0.0)
        {
            return Err("SCENARIO_RATE_OUT_OF_RANGE");
        }
        let current = eta(b.current_rate_per_second);
        let proposed = eta(proposed_rate);
        rows.push(serde_json::json!({"bucket":b.id,"unit":b.unit,"current_seconds":current,"proposed_seconds":proposed,
            "estimated_extension_seconds":current.zip(proposed).and_then(|(a,c)|{let d=c-a;d.is_finite().then_some(d)}),"exhausts_before_reset":proposed.zip(b.resets_in_seconds).map(|(a,r)|a<r),"beyond_reset_unsupported":proposed.zip(b.resets_in_seconds).is_some_and(|(a,r)|a>=r)}));
    }
    Ok(
        serde_json::json!({"simulation_only":true,"automatic_switching":false,"assumptions":assumptions,"buckets":rows,
        "warning":"No additive combined-quota claim. Multipliers are explicit assumptions, not measured routing conversions; no forecast after a reset."}),
    )
}

/// Documented external payloads are accepted only via an explicit local import.
/// Provider-reported numeric precision does NOT upgrade the import's provenance.
pub fn normalize_vendor(
    format: &str,
    payload: &serde_json::Value,
    provider_version: &str,
    account: &str,
    at: i64,
) -> Check<Vec<QuotaSample>> {
    control::id(account)?;
    control::text(provider_version, 80)?;
    control::timestamp(at)?;
    let mut out = Vec::new();
    let mut add = |provider: &str,
                   bucket: String,
                   used: f64,
                   reset: Option<i64>,
                   meter: Meter,
                   unit: &str,
                   limit: Option<f64>|
     -> Check<()> {
        let s = QuotaSample {
            id: format!("sample-{}", out.len()),
            provider: provider.into(),
            account_alias: account.into(),
            bucket,
            meter,
            unit: unit.into(),
            window_id: reset
                .map(|r| format!("reset-{r}"))
                .unwrap_or_else(|| format!("session-{at}")),
            used,
            limit,
            reset_at: reset,
            observed_at: at,
            precision: "provider_reported".into(),
            evidence: Evidence {
                kind: EvidenceKind::AgentReported,
                source: format!("explicit-import:{format}:{provider_version}"),
                at,
                coverage: Coverage::Partial,
            },
        };
        s.validate()?;
        out.push(s);
        Ok(())
    };
    match format {
        "codex-rate-limits-2026-09" => {
            let root = payload.get("result").unwrap_or(payload);
            let entries: Vec<(&str, &serde_json::Value)> =
                if let Some(m) = root.get("rateLimitsByLimitId").and_then(|v| v.as_object()) {
                    m.iter().map(|(k, v)| (k.as_str(), v)).collect()
                } else if let Some(v) = root.get("rateLimits") {
                    vec![(
                        v.get("limitId").and_then(|v| v.as_str()).unwrap_or("codex"),
                        v,
                    )]
                } else {
                    return Err("VENDOR_QUOTA_ABSENT");
                };
            if entries.len() > 32 {
                return Err("BUCKET_LIMIT");
            }
            for (id, value) in entries {
                for window in ["primary", "secondary"] {
                    if let Some(w) = value.get(window).filter(|v| !v.is_null()) {
                        let used = w
                            .get("usedPercent")
                            .and_then(|v| v.as_f64())
                            .ok_or("VENDOR_USED_UNKNOWN")?;
                        let reset = w.get("resetsAt").and_then(|v| v.as_i64());
                        add(
                            "codex",
                            format!("{id}-{window}"),
                            used,
                            reset,
                            Meter::Subscription,
                            "percent",
                            Some(100.0),
                        )?;
                    }
                }
            }
        }
        "claude-statusline-2026-09" => {
            for window in ["five_hour", "seven_day", "spend_limit"] {
                if let Some(w) = payload
                    .get("rate_limits")
                    .and_then(|r| r.get(window))
                    .filter(|v| !v.is_null())
                {
                    let used = w
                        .get("used_percentage")
                        .and_then(|v| v.as_f64())
                        .ok_or("VENDOR_USED_UNKNOWN")?;
                    add(
                        "claude",
                        window.into(),
                        used,
                        w.get("resets_at").and_then(|v| v.as_i64()),
                        Meter::Subscription,
                        "percent",
                        Some(100.0),
                    )?;
                }
            }
            if let Some(u) = payload
                .get("context_window")
                .and_then(|v| v.get("used_percentage"))
                .and_then(|v| v.as_f64())
            {
                add(
                    "claude",
                    "context".into(),
                    u,
                    None,
                    Meter::Context,
                    "percent",
                    Some(100.0),
                )?;
            }
            // cost.total_cost_usd is an estimate for a session, NOT a subscription debit or bill.
            if let Some(u) = payload
                .get("cost")
                .and_then(|v| v.get("total_cost_usd"))
                .and_then(|v| v.as_f64())
            {
                add(
                    "claude",
                    "session-estimated-cost".into(),
                    u,
                    None,
                    Meter::Dollars,
                    "usd",
                    None,
                )?;
                if let Some(last) = out.last_mut() {
                    last.precision = "estimated".into();
                }
            }
        }
        _ => return Err("VENDOR_FORMAT_UNSUPPORTED"),
    }
    if let Some(session) = payload.get("session_id").and_then(|v| v.as_str()) {
        control::text(session, 256)?;
        for sample in &mut out {
            if sample.reset_at.is_none() {
                sample.window_id = format!("session-{session}");
            }
        }
    }
    if out.is_empty() {
        return Err("VENDOR_QUOTA_ABSENT");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn s(t: i64, u: f64) -> QuotaSample {
        QuotaSample {
            id: format!("s{t}"),
            provider: "test".into(),
            account_alias: "personal".into(),
            bucket: "weekly".into(),
            meter: Meter::Subscription,
            unit: "percent".into(),
            window_id: "w1".into(),
            used: u,
            limit: Some(100.0),
            reset_at: Some(200000),
            observed_at: t,
            precision: "exact".into(),
            evidence: Evidence {
                kind: EvidenceKind::Observed,
                source: "synthetic".into(),
                at: t,
                coverage: Coverage::Complete,
            },
        }
    }
    #[test]
    fn sparse_is_unknown() {
        assert!(forecast(&[s(100, 2.0)], 100).unwrap().horizons.is_empty());
    }
    #[test]
    fn known_linear_rate() {
        let r = forecast(&[s(100, 10.0), s(3700, 20.0)], 3700).unwrap();
        let b = r.horizons.last().unwrap();
        assert!((b.exhaustion_after_seconds.unwrap() - 28800.0).abs() < 1e-6);
    }
    #[test]
    fn reset_does_not_bridge() {
        let mut b = s(4000, 2.0);
        b.window_id = "w2".into();
        assert!(forecast(&[s(100, 90.0), b], 4000)
            .unwrap()
            .horizons
            .is_empty());
    }
    #[test]
    fn counter_correction_does_not_negative_burn() {
        assert!(forecast(&[s(100, 90.0), s(4000, 2.0)], 4000)
            .unwrap()
            .horizons
            .is_empty());
    }
    #[test]
    fn stale_withholds_eta() {
        assert!(forecast(&[s(100, 1.0), s(200, 2.0)], 2000)
            .unwrap()
            .horizons
            .is_empty());
    }
    #[test]
    fn no_burn_not_infinite_claim() {
        let r = forecast(&[s(100, 1.0), s(200, 1.0)], 200).unwrap();
        assert!(r
            .horizons
            .iter()
            .all(|h| h.exhaustion_after_seconds.is_none()));
    }
    #[test]
    fn duplicate_times_rejected() {
        assert!(forecast(&[s(100, 1.0), s(100, 2.0)], 100).is_err());
    }
    #[test]
    fn mixed_accounts_rejected() {
        let mut b = s(200, 2.0);
        b.account_alias = "other".into();
        assert!(forecast(&[s(100, 1.0), b], 200).is_err());
    }
    #[test]
    fn costs_not_quota() {
        let mut a = s(100, 1.0);
        a.meter = Meter::Dollars;
        a.unit = "usd".into();
        assert!(forecast(&[a], 100).unwrap().horizons.is_empty());
    }
    #[test]
    fn unknown_capacity_kept_unknown() {
        let mut a = s(100, 1.0);
        a.limit = None;
        assert!(forecast(&[a], 100).unwrap().remaining.is_none());
    }
    #[test]
    fn overdue_reset_requires_observation() {
        let mut a = s(100, 1.0);
        a.reset_at = Some(200);
        assert!(forecast(&[a], 300).unwrap().horizons.is_empty());
    }
    #[test]
    fn quota_overrun_zero_remaining() {
        assert_eq!(
            forecast(&[s(100, 110.0)], 100).unwrap().remaining,
            Some(0.0)
        );
    }
    #[test]
    fn nonfinite_input_rejected() {
        assert!(forecast(&[s(100, f64::NAN)], 100).is_err());
    }
    #[test]
    fn plan_fit_needs_three_cycles() {
        assert_eq!(plan_fit(&[]).unwrap()["status"], "unknown");
    }
    #[test]
    fn licensing_is_not_optional() {
        let m = ModelMetric {
            model: "m".into(),
            benchmark: "b".into(),
            revision: "r".into(),
            quality: 1.0,
            task_cost_usd: 1.0,
            latency_ms: None,
            available: true,
            license_state: "unknown".into(),
            source: "fixture".into(),
        };
        assert!(pareto(&[m]).is_err());
    }
    #[test]
    fn missing_vendor_window_stays_unknown() {
        assert!(normalize_vendor(
            "claude-statusline-2026-09",
            &serde_json::json!({}),
            "fixture",
            "account",
            100
        )
        .is_err());
    }
    #[test]
    fn imported_json_cannot_be_provider_verified() {
        let x = normalize_vendor(
            "claude-statusline-2026-09",
            &serde_json::json!({"rate_limits":{"five_hour":{"used_percentage":0,"resets_at":200}}}),
            "fixture",
            "account",
            100,
        )
        .unwrap();
        assert_eq!(x[0].evidence.kind, EvidenceKind::AgentReported);
        assert_eq!(x[0].used, 0.0);
    }
    #[test]
    fn scenario_requires_assumptions() {
        assert!(counterfactual(&[], &[]).is_err());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreferenceProfile {
    MaximumIntelligence,
    Balanced,
    Stretch,
    Economy,
}
impl PreferenceProfile {
    pub fn advisory(self) -> serde_json::Value {
        let (mode,policy)=match self{
   Self::MaximumIntelligence=>("maximum_intelligence","Prefer highest measured task-relevant quality; disclose quota and latency trade-offs."),
   Self::Balanced=>("balanced","Consider the task-relevant Pareto frontier without silently switching providers."),
   Self::Stretch=>("stretch","Preserve scarce subscription windows; simulate explicitly assumed routing changes only."),
   Self::Economy=>("economy","Prefer lower task cost above an explicit quality threshold; do not equate subscription quota with dollars."),
  };
        serde_json::json!({"mode":mode,"advice":policy,"automatic_switching":false,"subscription_changes":false})
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionRequest {
    pub models: Vec<ModelMetric>,
    pub criterion: String,
    pub quality_threshold: f64,
}
pub fn select_models(request: &SelectionRequest) -> Check<serde_json::Value> {
    if !request.quality_threshold.is_finite() {
        return Err("INVALID_QUALITY_THRESHOLD");
    }
    let frontier = pareto(&request.models)?;
    if ![
        "best_quality",
        "cheapest_above_threshold",
        "fastest_above_threshold",
    ]
    .contains(&request.criterion.as_str())
    {
        return Err("MODEL_CRITERION_UNSUPPORTED");
    }
    let mut candidates: Vec<_> = request
        .models
        .iter()
        .filter(|m| m.available && m.quality >= request.quality_threshold)
        .collect();
    match request.criterion.as_str() {
        "best_quality" => candidates.sort_by(|a, b| {
            b.quality
                .total_cmp(&a.quality)
                .then_with(|| a.task_cost_usd.total_cmp(&b.task_cost_usd))
                .then_with(|| a.model.cmp(&b.model))
        }),
        "cheapest_above_threshold" => candidates.sort_by(|a, b| {
            a.task_cost_usd
                .total_cmp(&b.task_cost_usd)
                .then_with(|| b.quality.total_cmp(&a.quality))
                .then_with(|| a.model.cmp(&b.model))
        }),
        _ => {
            candidates.retain(|m| m.latency_ms.is_some());
            candidates.sort_by(|a, b| {
                a.latency_ms
                    .unwrap_or(f64::MAX)
                    .total_cmp(&b.latency_ms.unwrap_or(f64::MAX))
                    .then_with(|| a.model.cmp(&b.model))
            });
        }
    }
    Ok(
        serde_json::json!({"candidates":candidates.iter().map(|m|&m.model).collect::<Vec<_>>(),"criterion":request.criterion,"threshold":request.quality_threshold,"pareto":frontier,"automatic_switching":false,"data_source":"explicit_licensed_import"}),
    )
}

#[cfg(test)]
mod audit_numeric_tests {
    use super::*;
    #[test]
    fn ratio_overflow_is_unknown_not_infinity() {
        assert_eq!(finite_ratio(1e15, 1e-310), None);
        assert_eq!(finite_ratio(10.0, 2.0), Some(5.0));
    }
    #[test]
    fn finite_inputs_cannot_produce_overflow_rate() {
        let b = ScenarioBucket {
            id: "b".into(),
            unit: "credits".into(),
            remaining: 1.0,
            current_rate_per_second: 1e308,
            proposed_rate_multiplier: 10.0,
            resets_in_seconds: None,
        };
        assert!(counterfactual(&[b], &["synthetic assumption".into()]).is_err());
    }
    fn cycle(account: Option<&str>, n: usize) -> Cycle {
        Cycle {
            provider: "p".into(),
            account_alias: account.map(str::to_owned),
            bucket: "b".into(),
            window_id: format!("w{n}"),
            utilization: 0.2,
            complete: true,
            evidence_ref: "synthetic".into(),
        }
    }
    #[test]
    fn unidentified_plan_fit_is_unknown() {
        let v = (0..3).map(|n| cycle(None, n)).collect::<Vec<_>>();
        assert_eq!(
            plan_fit(&v).unwrap()["reason"],
            "explicit_account_identity_required"
        );
    }
    #[test]
    fn plan_fit_rejects_mixed_accounts() {
        assert!(plan_fit(&[
            cycle(Some("a"), 1),
            cycle(Some("b"), 2),
            cycle(Some("a"), 3)
        ])
        .is_err());
    }
    #[test]
    fn safe_even_median_does_not_overflow() {
        let v = (0..4)
            .map(|n| {
                let mut c = cycle(Some("a"), n);
                c.utilization = 1e308;
                c
            })
            .collect::<Vec<_>>();
        assert!(plan_fit(&v).unwrap()["median_utilization"]
            .as_f64()
            .unwrap()
            .is_finite());
    }
}
