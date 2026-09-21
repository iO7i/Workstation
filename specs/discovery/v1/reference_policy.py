"""Development-only reference policy for FS-1.0-D1, not Workstation runtime.
Inputs are schema-validated synthetic/local metadata. No IO or authority is exercised.
"""
from __future__ import annotations
from datetime import datetime, timedelta, timezone
from urllib.parse import urlsplit, urlunsplit


def moment(value: str) -> datetime:
    result = datetime.fromisoformat(value.replace("Z", "+00:00"))
    if result.tzinfo is None:
        raise ValueError("timezone required")
    return result.astimezone(timezone.utc)


def canonical_source(value: str) -> str:
    parsed = urlsplit(value)
    if (parsed.scheme != "https" or not parsed.hostname or parsed.username is not None
            or parsed.password is not None or parsed.query
            or any(ord(c) < 33 for c in value)):
        raise ValueError("reference URL is not an allowed public HTTPS source")
    # This is identification only: do not resolve DNS, follow links, or fetch a URL.
    return urlunsplit((parsed.scheme, parsed.netloc.lower(), parsed.path.rstrip("/"), "", ""))


def recommend(context: dict, resources: list[dict], feedback: list[dict],
              now: str) -> dict:
    stamp = moment(now)
    empty = {"schema_version": 1, "source": "offline_local_catalog", "generated_at": now,
             "useful_discoveries": [], "catalog_status": "reviewed_local" if resources else "empty",
             "limitations": ["Offline catalog only; no online search or action was performed."]}
    if (not context["focus_approved"] or not context["needs"]
            or not context["evidence_refs"]):
        return empty
    if len(resources) > 100:
        raise ValueError("reference policy input budget exceeded")
    if len({r["id"] for r in resources}) != len(resources):
        raise ValueError("duplicate resource identity")
    inventory = {}
    for observation in context["inventory"]:
        key = observation["resource_id"]
        if key in inventory:
            raise ValueError("conflicting inventory must be reconciled, not overwritten")
        inventory[key] = observation["state"] if observation["coverage"] == "complete" else "unknown"
    aliases = {r["id"]: canonical_source(r["canonical_url"]) for r in resources}
    latest = {}
    for item in feedback:
        if item["project_id"] != context["project_id"] or moment(item["recorded_at"]) > stamp:
            continue
        key = (aliases.get(item["resource_id"], item["resource_id"]), item["need"])
        if key not in latest or (moment(item["recorded_at"]), item["feedback_id"]) > (
                moment(latest[key]["recorded_at"]), latest[key]["feedback_id"]):
            latest[key] = item
    constraints = context["constraints"]
    blocked_sources = {aliases[i] for i in constraints["blocked_resource_ids"] if i in aliases}
    chosen = []
    for r in resources:
        canonical = aliases[r["id"]]
        if r["id"] in constraints["blocked_resource_ids"] or canonical in blocked_sources:
            continue
        if r["review"]["status"] != "reviewed":
            continue  # A lead is not a normal recommendation.
        compat = r["compatibility"]
        if compat["platforms"] and context["platform"] not in compat["platforms"] and "platform_neutral" not in compat["platforms"]:
            continue
        if constraints["free_only"] and compat["cost"] == "paid":
            continue
        if constraints["local_only"] and compat["privacy"] == "external":
            continue
        if not constraints["allow_network_dependency"] and compat["requires_network"]:
            continue
        matching = sorted(set(context["needs"]) & set(r["needs"]))
        matching = [n for n in matching if not (
            (f := latest.get((canonical, n))) and (
                f["disposition"] == "dismissed" or (f["disposition"] == "snoozed" and moment(f["until"]) > stamp)))]
        if not matching:
            continue
        need = matching[0]
        review = r["review"]
        if moment(review["reviewed_at"]) > stamp:
            continue  # A future review cannot establish current reviewed status.
        stale = stamp - moment(review["reviewed_at"]) > timedelta(days=review["fresh_for_days"])
        unknown = (compat["evidence_status"] == "unknown" or not compat["platforms"]
                   or bool(compat["version_requirements"])
                   or r["license"]["status"] != "documented"
                   or (constraints["free_only"] and compat["cost"] == "unknown")
                   or (constraints["local_only"] and compat["privacy"] == "unknown"))
        available = inventory.get(r["id"], "unknown")
        f = latest.get((canonical, need))
        benefit = "potential"
        if (f and f["disposition"] == "evaluated" and f["evidence_ref"]
                and f["catalog_revision"] == review["reviewed_revision"]):
            benefit = {"useful": "observed_useful_here", "not_useful": "evaluated_not_useful_here",
                       "inconclusive": "inconclusive_here"}.get(f["outcome"], "potential")
        card = {"resource_id": r["id"], "title": r["title"], "kind": r["kind"],
                "canonical_url": r["canonical_url"], "matched_need": need,
                "evidence_refs": list(context["evidence_refs"]),
                "why_now": "Matches the approved project need: " + need,
                "potential_benefit": r["description"], "limitations": list(r["limitations"]),
                "availability": available, "review_freshness": "stale" if stale else "current",
                "reviewed_revision": review["reviewed_revision"],
                "compatibility_status": "needs_review" if unknown or stale else "documented",
                "benefit_status": benefit, "license": dict(r["license"]),
                "next_step": "review_applicability" if unknown or stale else
                             "use_existing_reference" if available == "available" else "read_and_evaluate",
                "adoption_mode": r["adoption"]["mode"], "authorized_actions": []}
        if stale:
            card["limitations"].append("Catalog review is stale; reconfirm applicability before adoption.")
        if unknown:
            card["limitations"].append("One or more applicability constraints have not been verified.")
        # Equivalence is explicitly curated; a shared need is not proof of equivalence.
        sort_key = (card["compatibility_status"] != "documented", available != "available",
                    stale, -len(matching), r["id"])
        chosen.append((sort_key, canonical, r["equivalence_key"], card))
    chosen.sort(key=lambda item: item[0])
    sources, equivalents = set(), set()
    for _, canonical, equivalent, card in chosen:
        equivalence_scope = (equivalent, card["matched_need"])
        if canonical in sources or (equivalent and equivalence_scope in equivalents):
            continue
        sources.add(canonical)
        if equivalent:
            equivalents.add(equivalence_scope)
        empty["useful_discoveries"].append(card)
        if len(empty["useful_discoveries"]) == 3:
            break
    return empty
