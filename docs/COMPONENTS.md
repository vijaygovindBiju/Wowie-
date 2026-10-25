# Workspace Component Contracts

**Status:** Implemented across the 8 crates in the Wowie Cargo workspace. All components communicate via strictly-typed Rust contracts with `#![forbid(unsafe_code)]`.


| Component | Responsibilities and I/O | Dependencies / state owner | Failure behavior | Test strategy |
|---|---|---|---|---|
| Goal and commitment domain | Validated user goals, next actions, commitment schedules; emits domain events. | Clock, repository; owns goal/session state. | Reject invalid changes; never rewrite user goal automatically. | Pure domain tests, lifecycle tests. |
| Activity observation | Read configured app/window metadata; report capability, consent, status, observation events. | Platform adapter; owns only transient source state. | Denied/unavailable means no observation and visible status. | Adapter contract tests on named desktop sessions. |
| Browser connector (optional) | Deliver minimized URL/origin metadata after separate permission. | Browser extension + local handshake; transient state. | Missing/disconnected connector becomes unknown; no fabricated URL. | Permission and browser compatibility tests. |
| Activity normalizer/segmenter | Bound, redact, timestamp and group raw events into activity segments. | Observation source, clock; owns transient buffers. | Drop malformed/stale events; emit diagnostic without sensitive payload. | Fixture, boundary, and timing tests. |
| Classifier/goal assessment | Combine explicit rules, user corrections, and optional AI assessment; return label/confidence/evidence. | Goals, normalized segments, AI interface; owns no enforcement policy. | Unknown/ambiguous on errors; never imply certainty. | Labeled fixtures and adversarial input tests. |
| AI provider adapter | Send the minimum configured context and parse structured output. | Local/cloud provider, credential store; no durable memory by default. | Timeout/malformed output returns unavailable. | Contract, timeout, privacy-egress, injection tests. |
| Deterministic policy engine | Apply consent, thresholds, break/cooldown, allowed action, expiry, and rate limits. | Domain state and assessment; stateless decision plus explicit policy state. | Deny unsafe/invalid actions. | Table-driven tests with fake clock; no OS required. |
| Coaching/notification delivery | Turn allowed decision into concise, uncertainty-aware message and user choices. | Policy event, notification API; owns delivery status. | Notification failure leaves session active and controls accessible. | Renderer/notification fakes and UI tests. |
| Character registry/import | Validate supported formats, manage metadata/profile/assets, expose capability report. | Sandboxed/bounded parser, asset store; owns asset inventory. | Reject safely and preserve notification fallback. | Malformed, oversized, extension, license metadata corpus. |
| Character renderer | Display model and map presentation events to available idle/expression/gesture. | Validated asset, graphics runtime; owns transient pose/render state. | Crash/unsupported animation falls back without affecting policy. | Deterministic event mapping, renderer smoke/resource tests. |
| Personality/context | Store name, tone, and user-approved preferences; generate bounded wording context. | Settings; no independent memory store. | Fall back to neutral templates. | Prompt/output schema tests and settings tests. |
| Enforcement authorization | Validate explicit user confirmation and issue narrow, expiring typed request. | Policy, UI, supported adapter; owns authorization/audit state. | Deny absent/expired/ambiguous consent. | State-machine and negative tests. |
| Enforcement adapter | Apply/remove only supported reversible action. | OS/browser integration; owns active action state. | Stop/expiry cleanup; surface unresolved cleanup and manual steps. | Fake adapter plus named platform fault tests. |
| Persistence/history | Store minimal durable records, migrations, retention, export, delete. | Local database, asset directory; owns durable records. | Transaction failure surfaces; do not pretend saved data exists. | Migration, crash, deletion, retention tests. |
| Settings/configuration | Validate user preferences and safe defaults. | Persistence and OS capabilities. | Invalid settings revert to safe defaults with explanation. | Schema and migration tests. |
| Diagnostics (optional) | Local redacted technical diagnostics; no activity telemetry by default. | App events; opt-in export. | Suppress sensitive values; no cloud upload by default. | Redaction and export review. |

