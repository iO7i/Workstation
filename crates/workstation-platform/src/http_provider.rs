//! Narrow documented GET adapters. Never scrapes browser sessions or accepts arbitrary auth URLs.
use crate::{storage::Store, tasks, vault::SecretBytes, Error, Result};
use reqwest::{
    blocking::{Client, Response},
    header::{HeaderMap, HeaderValue},
};
use serde_json::{json, Value};
use std::{
    io::Read,
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    sync::mpsc,
    time::{Duration, Instant},
};
use workstation_core::{
    control::{EvidenceKind, Resource, ResourceKind},
    effects::BillingProvider,
    usage_import,
};
fn builder() -> reqwest::blocking::ClientBuilder {
    Client::builder()
        .no_proxy()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .referer(false)
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .user_agent(concat!("workstation/", env!("CARGO_PKG_VERSION")))
}
fn response_json(r: Response) -> Result<Value> {
    let status = r.status().as_u16();
    if status == 401 || status == 403 {
        return Err(Error::new("PROVIDER_AUTH_REJECTED"));
    }
    if status == 429 {
        return Err(Error::new("PROVIDER_RATE_LIMITED_NO_AUTORETRY"));
    }
    if !r.status().is_success() {
        return Err(Error::new("PROVIDER_HTTP_ERROR_BODY_SUPPRESSED"));
    }
    let mut b = SecretBytes(Vec::new());
    r.take(1024 * 1024 + 1)
        .read_to_end(&mut b.0)
        .map_err(|_| Error::new("PROVIDER_READ_FAILED"))?;
    if b.0.len() > 1024 * 1024 {
        return Err(Error::new("PROVIDER_BODY_LIMIT"));
    }
    serde_json::from_slice(&b.0).map_err(|_| Error::new("PROVIDER_JSON_SHAPE"))
}
fn headers(provider: BillingProvider, key: &SecretBytes) -> Result<HeaderMap> {
    let s = std::str::from_utf8(&key.0).map_err(|_| Error::new("AUTH_ENCODING"))?;
    let mut h = HeaderMap::new();
    let value = if provider == BillingProvider::OpenAiCosts {
        format!("Bearer {s}")
    } else {
        s.to_owned()
    };
    let mut v = HeaderValue::from_str(&value).map_err(|_| Error::new("AUTH_HEADER_INVALID"))?;
    v.set_sensitive(true);
    if provider == BillingProvider::OpenAiCosts {
        h.insert("authorization", v);
    } else {
        h.insert("x-api-key", v);
        h.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
    }
    Ok(h)
}
#[expect(
    clippy::too_many_arguments,
    reason = "billing query keeps project, environment, provider, credential, account and bounded interval explicit"
)]
pub fn billing(
    store: &Store,
    project: &str,
    environment: &str,
    provider: BillingProvider,
    secret_id: &str,
    account: &str,
    start: i64,
    end: i64,
) -> Result<Value> {
    if start < 0
        || end <= start
        || end - start > 32 * 86400
        || end > crate::control_store::epoch() + 60
    {
        return Err(Error::new("BILLING_DATE_RANGE"));
    }
    let secret = tasks::resolve(store, project, environment, secret_id)?;
    let h = headers(provider, &secret)?;
    let client = builder()
        .default_headers(h)
        .build()
        .map_err(|_| Error::new("TLS_CLIENT_FAILED"))?;
    let (url, label, mut query) = if provider == BillingProvider::OpenAiCosts {
        (
            "https://api.openai.com/v1/organization/costs",
            "openai",
            vec![
                ("start_time".to_owned(), start.to_string()),
                ("end_time".into(), end.to_string()),
                ("bucket_width".into(), "1d".into()),
                ("limit".into(), "31".into()),
            ],
        )
    } else {
        let fmt = |t| {
            time::OffsetDateTime::from_unix_timestamp(t)
                .map_err(|_| Error::new("TIMESTAMP_RANGE"))?
                .format(&time::format_description::well_known::Rfc3339)
                .map_err(|_| Error::new("TIMESTAMP_ENCODING"))
        };
        (
            "https://api.anthropic.com/v1/organizations/cost_report",
            "anthropic",
            vec![
                ("starting_at".to_owned(), fmt(start)?),
                ("ending_at".into(), fmt(end)?),
                ("limit".into(), "31".into()),
            ],
        )
    };
    let began = Instant::now();
    let mut rows = vec![];
    let mut cursors = std::collections::BTreeSet::new();
    let mut pages = 0;
    loop {
        if pages >= 4 || began.elapsed() > Duration::from_secs(40) {
            return Err(Error::new("BILLING_PAGINATION_BUDGET"));
        }
        pages += 1;
        let body = response_json(
            client
                .get(url)
                .query(&query)
                .send()
                .map_err(|_| Error::new("PROVIDER_TRANSPORT_FAILED"))?,
        )?;
        let batch = usage_import::cost_page(label, &body).map_err(Error::new)?;
        if batch
            .iter()
            .any(|r| r.start < start - 86400 || r.end > end + 86400)
        {
            return Err(Error::new("COST_BUCKET_OUTSIDE_REQUEST"));
        }
        rows.extend(batch);
        let more = body
            .get("has_more")
            .and_then(Value::as_bool)
            .ok_or(Error::new("PAGINATION_SHAPE"))?;
        if !more {
            break;
        }
        let next = body
            .get("next_page")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty() && s.len() <= 2048 && !s.chars().any(char::is_control))
            .ok_or(Error::new("PAGINATION_TOKEN_INVALID"))?;
        if !cursors.insert(next.to_owned()) {
            return Err(Error::new("PAGINATION_CYCLE"));
        }
        query.retain(|(k, _)| k != "page");
        query.push(("page".into(), next.into()));
    }
    let mut summary = usage_import::cost_summary(&rows).map_err(Error::new)?;
    summary["provider"] = json!(label);
    summary["account_alias"] = json!(account);
    summary["observed_at"] = json!(crate::control_store::epoch());
    summary["source"] = json!("documented_organization_admin_api");
    summary["coverage"] = json!("partial");
    summary["scope_notice"]=json!("Organization-wide costs selected by this credential; not attributed to the Workstation project. Subscription quotas are separate.");
    if provider == BillingProvider::AnthropicCosts {
        summary["exclusions"] = json!(["Provider documents Priority Tier costs are not included."]);
    }
    Ok(summary)
}
fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => {
            let o = a.octets();
            !a.is_private()
                && !a.is_loopback()
                && !a.is_link_local()
                && !a.is_unspecified()
                && !a.is_multicast()
                && o[0] != 0
                && o[0] < 224
                && !(o[0] == 100 && (64..128).contains(&o[1]))
                && !(o[0] == 192 && o[1] == 0)
                && !(o[0] == 198 && (o[1] == 18 || o[1] == 19))
                && !(o[0] == 198 && o[1] == 51 && o[2] == 100)
                && !(o[0] == 203 && o[1] == 0 && o[2] == 113)
        }
        IpAddr::V6(a) => {
            if let Some(v4) = a.to_ipv4_mapped() {
                return public_ip(IpAddr::V4(v4));
            }
            let s = a.segments();
            (s[0] & 0xe000) == 0x2000
                && s[0] != 0x2002
                && !(s[0] == 0x2001 && (s[1] <= 0x1ff || s[1] == 0xdb8))
        }
    }
}
pub fn endpoint(r: &Resource) -> Result<Value> {
    if r.kind != ResourceKind::Endpoint
        || !matches!(
            r.evidence.kind,
            EvidenceKind::UserApproved | EvidenceKind::ProviderVerified
        )
    {
        return Err(Error::new("APPROVED_ENDPOINT_REQUIRED"));
    }
    let url = reqwest::Url::parse(&r.locator).map_err(|_| Error::new("ENDPOINT_URL_INVALID"))?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.port_or_known_default() != Some(443)
    {
        return Err(Error::new("PUBLIC_HTTPS_ENDPOINT_REQUIRED"));
    }
    let host = url
        .host_str()
        .ok_or(Error::new("ENDPOINT_HOST_MISSING"))?
        .to_owned();
    let (tx, rx) = mpsc::sync_channel(1);
    let lookup = host.clone();
    std::thread::spawn(move || {
        let value = (lookup.as_str(), 443)
            .to_socket_addrs()
            .map(|x| x.take(17).collect::<Vec<_>>());
        let _ = tx.send(value);
    });
    let ips: Vec<SocketAddr> = rx
        .recv_timeout(Duration::from_secs(3))
        .map_err(|_| Error::new("DNS_TIMEOUT"))?
        .map_err(|_| Error::new("DNS_FAILED"))?;
    if ips.is_empty() || ips.len() > 16 || ips.iter().any(|s| !public_ip(s.ip())) {
        return Err(Error::new("NONPUBLIC_ENDPOINT_ADDRESS_REJECTED"));
    }
    // Pin validated answers for this request; do not perform a second unvalidated lookup.
    let client = builder()
        .resolve_to_addrs(&host, &ips)
        .build()
        .map_err(|_| Error::new("ENDPOINT_CLIENT_FAILED"))?;
    let response = client
        .head(url)
        .send()
        .map_err(|_| Error::new("ENDPOINT_TRANSPORT_FAILED"))?;
    Ok(
        json!({"resource_id":r.id,"project_id":r.project_id,"environment":r.environment,"status_code":response.status().as_u16(),"dns_resolved_public":true,"http_responded":true,"provider_identity_verified":false,"production_authority_verified":false,"redirect_followed":false,"body_read":false,"observed_at":crate::control_store::epoch()}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_addresses_denied() {
        for s in [
            "127.0.0.1",
            "10.0.0.1",
            "169.254.169.254",
            "100.64.0.1",
            "::1",
            "::ffff:127.0.0.1",
            "2001:db8::1",
        ] {
            assert!(!public_ip(s.parse().unwrap()));
        }
    }
}
