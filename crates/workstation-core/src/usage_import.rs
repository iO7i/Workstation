//! Import contracts never invent a provider endpoint, quota or billing entitlement.
use crate::{
    control::{self, Check, Evidence, EvidenceKind},
    economics::{Meter, QuotaSample},
    Coverage,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostBucket {
    pub start: i64,
    pub end: i64,
    pub amount_usd_nanos: i64,
}
/// Exact decimal/scientific-notation scaling, with checked arithmetic and no floating
/// rounding. Billing uses nano-USD (USD scale=9; decimal cents scale=7).
/// Values smaller than this precision are rejected rather than rounded invisibly.
pub fn decimal_fixed(value: &str, scale: u32) -> Check<i64> {
    if value.len() > 64
        || value.is_empty()
        || value.starts_with('-')
        || value.starts_with('+')
        || scale > 12
    {
        return Err("COST_DECIMAL_FORMAT");
    }
    let pieces: Vec<_> = value.split(['e', 'E']).collect();
    if pieces.len() > 2 {
        return Err("COST_DECIMAL_FORMAT");
    }
    let exponent = if pieces.len() == 2 {
        pieces[1]
            .parse::<i32>()
            .map_err(|_| "COST_DECIMAL_FORMAT")?
    } else {
        0
    };
    if !(-18..=18).contains(&exponent) {
        return Err("COST_DECIMAL_RANGE");
    }
    let mut parts = pieces[0].split('.');
    let whole = parts.next().ok_or("COST_DECIMAL_FORMAT")?;
    let frac = parts.next().unwrap_or("");
    if parts.next().is_some()
        || whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !frac.bytes().all(|b| b.is_ascii_digit())
    {
        return Err("COST_DECIMAL_FORMAT");
    }
    let digits = format!("{whole}{frac}");
    let n = digits.parse::<i128>().map_err(|_| "COST_DECIMAL_RANGE")?;
    let shift = scale as i32 + exponent - frac.len() as i32;
    let scaled = if shift >= 0 {
        n.checked_mul(
            10_i128
                .checked_pow(shift as u32)
                .ok_or("COST_DECIMAL_RANGE")?,
        )
        .ok_or("COST_DECIMAL_RANGE")?
    } else {
        let divisor = 10_i128
            .checked_pow((-shift) as u32)
            .ok_or("COST_DECIMAL_RANGE")?;
        if n % divisor != 0 {
            return Err("COST_DECIMAL_PRECISION");
        }
        n / divisor
    };
    i64::try_from(scaled).map_err(|_| "COST_DECIMAL_RANGE")
}
pub fn parse_time(v: &str) -> Check<i64> {
    time::OffsetDateTime::parse(v, &time::format_description::well_known::Rfc3339)
        .map(|x| x.unix_timestamp())
        .map_err(|_| "TIMESTAMP_FORMAT")
}
pub fn cost_page(provider: &str, v: &Value) -> Check<Vec<CostBucket>> {
    let data = v
        .get("data")
        .and_then(Value::as_array)
        .ok_or("COST_DATA_MISSING")?;
    if data.len() > 32 {
        return Err("COST_BUCKET_LIMIT");
    }
    let mut rows = vec![];
    for b in data {
        let (start, end) = if provider == "openai" {
            (
                b.get("start_time")
                    .and_then(Value::as_i64)
                    .ok_or("COST_TIME_MISSING")?,
                b.get("end_time")
                    .and_then(Value::as_i64)
                    .ok_or("COST_TIME_MISSING")?,
            )
        } else if provider == "anthropic" {
            (
                parse_time(
                    b.get("starting_at")
                        .and_then(Value::as_str)
                        .ok_or("COST_TIME_MISSING")?,
                )?,
                parse_time(
                    b.get("ending_at")
                        .and_then(Value::as_str)
                        .ok_or("COST_TIME_MISSING")?,
                )?,
            )
        } else {
            return Err("COST_PROVIDER_UNSUPPORTED");
        };
        if start < 0 || end <= start {
            return Err("COST_TIME_RANGE");
        }
        let results = b
            .get("results")
            .and_then(Value::as_array)
            .ok_or("COST_RESULTS_MISSING")?;
        if results.len() > 1000 {
            return Err("COST_RESULT_LIMIT");
        }
        let mut sum = 0i64;
        for r in results {
            let amount = if provider == "openai" {
                let a = r.get("amount").ok_or("COST_AMOUNT_MISSING")?;
                if a.get("currency").and_then(Value::as_str) != Some("usd") {
                    return Err("CURRENCY_NOT_USD");
                }
                let n = a.get("value").ok_or("COST_VALUE_MISSING")?;
                decimal_fixed(&n.to_string(), 9)?
            } else {
                if r.get("currency").and_then(Value::as_str) != Some("USD") {
                    return Err("CURRENCY_NOT_USD");
                }
                decimal_fixed(
                    r.get("amount")
                        .and_then(Value::as_str)
                        .ok_or("COST_AMOUNT_MISSING")?,
                    7,
                )?
            };
            sum = sum.checked_add(amount).ok_or("COST_SUM_RANGE")?;
        }
        rows.push(CostBucket {
            start,
            end,
            amount_usd_nanos: sum,
        });
    }
    Ok(rows)
}
pub fn copilot_quota(v: &Value, account: &str, at: i64, version: &str) -> Check<Vec<QuotaSample>> {
    control::id(account)?;
    control::timestamp(at)?;
    control::text(version, 128)?;
    let map = v
        .get("quotaSnapshots")
        .and_then(Value::as_object)
        .ok_or("COPILOT_QUOTA_SNAPSHOTS_REQUIRED")?;
    if map.len() > 16 {
        return Err("QUOTA_BUCKET_LIMIT");
    }
    let mut rows = vec![];
    for (bucket, q) in map {
        control::id(bucket)?;
        let limit = q.get("entitlementRequests").and_then(Value::as_f64);
        let used = q
            .get("usedRequests")
            .and_then(Value::as_f64)
            .ok_or("QUOTA_USED_MISSING")?;
        let reset = q
            .get("resetDateISO")
            .or_else(|| q.get("resetDate"))
            .and_then(Value::as_str)
            .map(parse_time)
            .transpose()?;
        let limit = limit.filter(|v| *v > 0.0); // -1 denotes unlimited, not a negative quota or infinite derived time.
        let s = QuotaSample {
            id: format!("copilot-{bucket}-{at}"),
            provider: "copilot".into(),
            account_alias: account.into(),
            bucket: bucket.clone(),
            meter: Meter::Subscription,
            unit: "requests".into(),
            window_id: reset
                .map(|t| format!("reset-{t}"))
                .unwrap_or_else(|| "unknown-window".into()),
            used,
            limit,
            reset_at: reset,
            observed_at: at,
            precision: "provider_reported".into(),
            evidence: Evidence {
                kind: EvidenceKind::AgentReported,
                source: format!("explicit_import:copilot_sdk:{version}"),
                at,
                coverage: Coverage::Partial,
            },
        };
        s.validate()?;
        rows.push(s);
    }
    Ok(rows)
}
/// Portable strict CSV fallback for vendors without an authorized API. Never marks it live/exact.
/// RFC4180 quoting is intentionally not accepted; this simple numeric contract rejects commas in IDs.
pub fn quota_csv(csv: &str, at: i64) -> Check<Vec<QuotaSample>> {
    if csv.len() > 256 * 1024 {
        return Err("CSV_LIMIT");
    }
    let mut lines = csv.lines();
    if lines.next()
        != Some("provider,account,bucket,meter,unit,window,used,limit,reset_at,observed_at")
    {
        return Err("CSV_HEADER");
    }
    let mut out = vec![];
    for (n, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if n >= 1000 || line.contains('"') {
            return Err("CSV_ROW_LIMIT_OR_QUOTING");
        }
        let f: Vec<_> = line.split(',').collect();
        if f.len() != 10 {
            return Err("CSV_COLUMNS");
        }
        let meter = match f[3] {
            "context" => Meter::Context,
            "subscription" => Meter::Subscription,
            "dollars" => Meter::Dollars,
            _ => return Err("CSV_METER"),
        };
        let optional = |s: &str| -> Check<Option<f64>> {
            if s.is_empty() {
                Ok(None)
            } else {
                Ok(Some(s.parse().map_err(|_| "CSV_NUMBER")?))
            }
        };
        let observed = f[9].parse::<i64>().map_err(|_| "CSV_TIME")?;
        if observed > at + 60 {
            return Err("FUTURE_USAGE_IMPORT");
        }
        let row = QuotaSample {
            id: format!("csv-{at}-{n}"),
            provider: f[0].into(),
            account_alias: f[1].into(),
            bucket: f[2].into(),
            meter,
            unit: f[4].into(),
            window_id: f[5].into(),
            used: f[6].parse().map_err(|_| "CSV_NUMBER")?,
            limit: optional(f[7])?,
            reset_at: if f[8].is_empty() {
                None
            } else {
                Some(f[8].parse().map_err(|_| "CSV_TIME")?)
            },
            observed_at: observed,
            precision: "estimated".into(),
            evidence: Evidence {
                kind: EvidenceKind::AgentReported,
                source: "explicit_csv_import_not_authenticated_provider".into(),
                at,
                coverage: Coverage::Partial,
            },
        };
        row.validate()?;
        out.push(row);
    }
    Ok(out)
}
pub fn cost_summary(rows: &[CostBucket]) -> Check<Value> {
    let mut ordered = rows.to_vec();
    ordered.sort_by_key(|r| r.start);
    for pair in ordered.windows(2) {
        if pair[0].end > pair[1].start {
            return Err("OVERLAPPING_COST_BUCKETS");
        }
    }
    let mut total = 0i64;
    let mut seen = std::collections::BTreeSet::new();
    for r in rows {
        if r.start < 0 || r.end <= r.start || r.amount_usd_nanos < 0 {
            return Err("INVALID_COST_BUCKET");
        }
        if !seen.insert((r.start, r.end)) {
            return Err("DUPLICATE_COST_BUCKET");
        }
        total = total
            .checked_add(r.amount_usd_nanos)
            .ok_or("COST_SUM_RANGE")?;
    }
    Ok(
        json!({"meter":"dollars","currency":"usd","amount_usd_nanos":total,"buckets":rows,"subscription_quota":null,"invoice_finality":"provider_cost_report_not_final_invoice"}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn currency_scales_differ() {
        assert_eq!(decimal_fixed("12.34", 4), Ok(123400));
        assert_eq!(decimal_fixed("12.34", 6), Ok(12340000));
    }
    #[test]
    fn excessive_precision_fails() {
        assert!(decimal_fixed("0.0000001", 6).is_err());
    }
    #[test]
    fn non_numbers_fail() {
        for x in ["NaN", "inf", "1e9999", "-1", "+2"] {
            assert!(decimal_fixed(x, 6).is_err());
        }
    }
    #[test]
    fn quota_not_billing() {
        let v = json!({"quotaSnapshots":{"chat":{"entitlementRequests":-1,"usedRequests":5,"resetDate":"2026-09-30T00:00:00Z"}}});
        let r = copilot_quota(&v, "a", 100, "fixture").unwrap();
        assert_eq!(r[0].limit, None);
        assert_eq!(r[0].meter, Meter::Subscription);
    }
}
