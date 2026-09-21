# FS-1.0-D1 — Contextual Resource Discovery

**Disposition:** accepted scope amendment, authorized by the user's instruction to add the attached proposal.  
**Date:** 18 September 2026.  
**Module:** Capabilities; human section: **Useful discoveries**.  
**Implementation:** R4; full integration/safety certification: R5. **R0 is unchanged.**  
**Current delivery:** normative scope, authoring contracts, synthetic fixtures, executable policy reference and implementation acceptance cases. No discovery feature is wired into the R0 Rust binary.

## 1. The promise

Given what the user is working on, surface a small number of existing resources that could materially help—including resources the user did not know to search for. A broken tool is not a prerequisite. Workstation remains the same five-module local application, not another agent, recommendation feed, marketplace, package manager or search platform.

Eligible resources include existing features, specialist tools/integrations, agent skills, reusable workflows, documentation, reference implementations, datasets and benchmarks. They must relate to registered development or research projects. Describing a dataset does not add dataset infrastructure; describing a skill does not activate it.

## 2. Separation of responsibility

| Module | Meaning of its record |
|---|---|
| Capabilities | This resource might help; here is why and what remains unverified. |
| Resource Atlas | The user explicitly chose to associate this resource with this project. |
| Chronicle | The user approved this reason for adopting, evaluating or rejecting it. |

A recommendation, saved card, model-generated candidate or valid JSON payload grants no authority. A saved association and an accepted decision are separate explicit operations. Adoption feedback does not install anything or accept a Chronicle decision. Never overwrite accepted project decisions from recommendations.

## 3. Inputs and evidence

Reuse approved project metadata, an optional approved current focus, explicit task needs, applicable decisions, inventory coverage, supported health findings and voluntary scoped feedback. The R4 decision engine receives these through the existing evidence/context model.

An initial small need vocabulary covers parallel work, interface review, accessibility review, reproducible evaluation, decision context, credential location, runtime recovery and storage attribution. Add needs through reviewed versioned catalog changes; do not hard-code product names as needs.

A technology marker alone does not establish a need. React is not proof of a poor interface. Docker is not evidence that the user should switch engines. No approved focus or applicable supported need can correctly produce zero suggestions. Incomplete inventory is **unknown**, not absent. A deliberately shared MCP or existing provider is not a reason to install a duplicate.

## 4. Matching and presentation

Apply this deterministic order:

1. Check explicit project scope, accepted decisions and constraints first. Exclude known platform, budget, privacy or dependency conflicts. Unknown compatibility yields `needs_review`, not an install-ready promise.
2. Require at least one supported need and source evidence. Do not use stars, popularity, affiliate fees or a desire to fill a quota as a reason.
3. Prefer relevant already-available capabilities or adopted resources over equivalent new dependencies, once their applicability is supported. Need overlap is not proof of equivalence: use a reviewed equivalence record.
4. Apply project/need-scoped dismissal and snoozing; deduplicate canonical resources and known equivalent options. Changing aliases or catalog revisions must not silently evade dismissal.
5. Return at most **three** useful cards. Never pad with weak matches. An empty result is valid.

Each card supplies its source, resource kind, why now, supporting evidence references, potential benefit, existing-availability coverage, resource-specific compatibility/license limitations, review freshness, smallest next step and **no authorized actions**. A resource can be reference-only even if an optional adapter exists elsewhere.

Distinguish three claims: a source exists; its stated purpose appears applicable; a scoped trial helped this project. Installation, download count and a successful JSON parse do not establish usefulness. A trial outcome needs its project, need, resource revision and evidence. Revisions require renewed applicability review.

## 5. Proactivity without a daemon

Show a bounded `useful_discoveries` field during ordinary project context retrieval, a project overview and a newly declared task. A health report may offer a relevant prevention reference after the diagnostic result, but optional discovery must not delay incident diagnosis.

All ordinary context/report queries use local approved data only. No synchronous web search, remote recommendation API, transcript import, model call, credential lookup or new scanner runs on that path. Without an invocation or an explicitly configured existing scheduled check, Workstation does not observe new activity.

Preserve the existing response/time budget: cached project context remains bounded. Drop optional suggestions with an explicit coverage/status indication rather than blocking essential project context.

**Name distinction:** R0's existing `discover` command enumerates known filesystem-root candidates. It is not the Useful discoveries feature and must not be advertised as it.

## 6. Two catalog layers

### A. Reviewed local catalog

Ship about **20–30 actually reviewed resources** at R4 launch, organized by needs. This is a curation target, not an integration count or permanent ceiling. Each entry has a canonical identity/source, publisher/revision evidence, resource kind, intended need, positive conditions, contraindications, limitations, resource-specific licensing, supported environment/version evidence, review time and adoption mode.

A catalog update can add descriptive resources without adding executable operations. Unknown license or compatibility stays unknown. Stale reviews remain usable as labeled references, never newly approved integrations. A famous repository is not one universal license/compatibility badge.

This amendment includes synthetic `example.org` fixtures only. **It does not claim that 20–30 real resources were researched or reviewed.** Curation is an explicit R4 gate. Sources cited in the user's supplied research are leads, not a certified current catalog.

### B. Optional research handoff

When local coverage is insufficient, reuse the user's browsing-capable agent:

`preview research brief -> user approves disclosure -> export brief -> agent researches externally -> preview/import candidates -> unverified review queue -> local review`

Workstation does not launch or orchestrate that agent. It does not acquire a native web-search dependency. A direct-search adapter is not required for v1.

Build the default brief from an allowlist of need tags, resource categories, platform and constraints. Omit private project names, paths, endpoints, repositories, transcripts, environment values, credentials and decisions. Even an allowlisted need can reveal something about work: require human review before external disclosure. A serialized schema field cannot prove disclosure approval; the future exporter binds local approval to the exact brief digest.

Import uses an explicit local project chosen by the user, bounded UTF-8 JSON (initial hard cap 1 MiB / 50 candidates), strict keys and length limits, validated HTTPS references, no link fetching and no archive extraction. Reject unexpected authority/install/script fields rather than silently accepting them. Do not resolve URLs during validation. Scheme/shape checks are not a complete privacy or network-safety guarantee.

Imported descriptions, titles and claimed compatibility are untrusted. The importer assigns IDs, digest, arrival time, review status and `reference_only` mode locally. It cannot accept caller-supplied `reviewed`, `certified_adapter`, approval, secret or command fields. No auto-promotion into the reviewed catalog. Canonical deduplication must not let a lead replace a reviewed resource or defeat a rejection.

## 7. Skill and integration boundary

Discovered material stays inert metadata. Never write a skill into a watched agent directory, load its scripts, paste its full instructions into an active agent prompt or follow its installation steps merely because it was found.

Existing `plan -> preview -> approve -> revalidate -> execute -> verify` governs a certified integration. An approval binds to exact version/revision/content and configuration diff. Changed targets invalidate approval. Pinning identifies reviewed content; it is not a claim of safety. Unsupported integrations remain reference-only.

Read-only MCP may expose cached suggestions and a sanitized draft research brief. It cannot import candidates, save feedback, accept a project decision, install a resource, connect accounts, resolve credentials or invoke an installer. Returned text is untrusted reference data. HTML/terminal renderers escape text and expose control-character issues.

## 8. Feedback and persistence

Persist small scoped dispositions: `saved`, `dismissed`, `snoozed`, `evaluated`, `adopted`, with project, canonical resource identity, need, timestamp, reason, catalog revision and optional evidence/outcome. Do not apply one project's rejection globally. Honor a snooze until its explicit time; do not undo a dismissal just because the upstream release changed.

Reuse existing projects, resources, evidence/provenance and source pointers. One small disposition/event table is sufficient when R4 is implemented. Keep unreviewed imports in a separately bounded local review area. No second project graph, vector database, recommender service or new event broker is authorized.

**No R0 migration is introduced by this amendment.** Candidate/disposition persistence must be integrated with the then-current schema and migration tests in R4, not added pre-emptively to `schema/001_r0.sql`.

## 9. Interfaces planned for R4 (not R0 commands)

Reuse `context`, project reports and capability views. The intended operations are list/explain suggestions, preview/export a research brief, preview/import candidates, record scoped feedback, review candidate metadata and request an existing integration plan. Exact CLI argument spelling must be contract-tested when implemented.

`specs/discovery/v1/` contains six strict draft authoring contracts and synthetic examples: reviewed catalog resource, private project context, local feedback, recommendation packet, allowlisted research brief, and untrusted candidate import. They are isolated from current R0 output contracts.

The executable reference policy is **development/test-only**. It illustrates need/constraint checks, three-card limits, existing-resource preference, stale labeling, uncertain inventory, dismissal, and separation of observed usefulness. Port/reconcile it into typed Rust when R4 is built. It is not a supported runtime API or a substitute for importer, database, UI, MCP and Windows tests.

## 10. Delivery and acceptance

- R0-A: unchanged Rust behavior, crate count, applied schema, commands, worker boundaries and runtime dependencies.
- R1–R3: preserve the existing build sequence and provide the approved context, inventories, policies and resource evidence that discovery will consume.
- R4: implement policy, 20–30 reviewed entries, scoped state, cached-context/report surfaces, minimal handoff/import and local feedback.
- R5: certify performance, offline operation, malformed/malicious imports, rendering/MCP authority boundaries, revision revalidation and coexistence.

See `FS-1.0-D1-ACCEPTANCE.md`. Every implementation case starts as **not run**. Development-only schema/reference-policy test passes are reported separately and never added to native/Rust/product pass counts.

Measure meaningful assistance: useful scoped trial outcomes, reasons for rejection, time/work avoided when observed and nuisance suppression. Do not optimize for clicks, star counts, installed tools or the number of suggestions.

## 11. Source and authority

This amendment follows the user-supplied `Pasted markdown(20260918-141358).md`, especially its resource categories, R4 placement, three-suggestion bound, optional research handoff and separation from authority. The user's current instruction accepts that proposed addition. No source facts about current prices, package versions, license terms or tool effectiveness were independently re-researched for this amendment.

Where illustrative research, a JSON shape or the reference code conflicts with an existing safety rule, block activation and reconcile before release. This addendum broadens discovery, not operational authority.
