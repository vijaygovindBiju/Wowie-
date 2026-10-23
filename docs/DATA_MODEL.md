# Proposed data model

This is a logical model, not a committed schema. SQLite is recommended but awaiting stack approval. Use stable opaque IDs, UTC timestamps, versioned records, and explicit provenance. Retention durations remain an owner decision.

## Entities

| Entity | Important fields | Relationships / validation |
|---|---|---|
| Ambition | id, title, optional description, created_at, updated_at, archived_at | User-authored broad intention; bounded text; no inferred personality or health label. |
| Goal | id, ambition_id, title, next_action, status, dates | Belongs to ambition; active/paused/completed/archived; user confirms AI suggestions. |
| Commitment | id, goal_id, planned_start, duration_seconds, break_allowance, monitoring_profile_id | Duration and breaks bounded by settings; explicit user creation. |
| FocusSession | id, commitment_id, started_at, ended_at, state, end_reason, consent snapshot | State transitions validated; one active session per profile unless later decided. |
| Observation | id, session_id, observed_at, source, app identifier, optional title/origin, idle flag | Minimal and preferably transient; raw content is not a durable default. Source and permission scope recorded. |
| ActivitySegment | id, session_id, start/end, app/origin reference, kind, confidence, evidence refs | Normalized interval; no content claim beyond available source. |
| Classification | id, segment_id, label, confidence, basis, classifier version, created_at | Distinguish deterministic rule, AI suggestion, and user correction; retain uncertainty. |
| CoachingDecision | id, session_id, decision type, policy version, evidence IDs, allowed choices, timestamp | Policy-produced; no action unless authorized separately. |
| UserAction | id, decision_id, action type, details, timestamp | Confirmed user response; correction scope is explicit. |
| CharacterProfile | id, display name, asset ID, personality settings, enabled, renderer profile | Independent of goal/policy; can be absent. |
| ModelAsset | id, managed path, format/version, size, hash, validation report, license metadata, imported_at | Untrusted input; never executable; deletion removes managed copy. |
| EnforcementPolicy | id, target rule, allowed action, max duration, consent requirement, enabled | User-authored; explicit targets only; no broad hidden defaults. |
| EnforcementAction | id, session_id, adapter, target scope, authorized_at, expires_at, state, stop reason | Must have confirmation, expiry, and stop capability; never inferred from AI alone. |
| RecoveryRecord | id, session_id, outcome, user note, next plan, timestamp | Optional and nonjudgmental; no punitive score. |
| EventHistory | id, event type, entity refs, timestamp, minimal payload, schema version | Audit useful decisions/actions; redact sensitive strings by default. |
| Settings | key/versioned structured values | User scope; safe default and validation; secrets stored outside ordinary settings. |

## Provenance and trust boundaries

Store three categories distinctly: (1) raw observations from a named source, (2) system/AI interpretations with version, confidence, and evidence references, and (3) confirmed user decisions/corrections. An AI label never overwrites the source event or becomes a confirmed user fact. A correction can supersede a classification with its scope without erasing the audit basis.

## Relationships and lifecycle

Ambition → goals → commitments → sessions. Sessions reference observations/segments, classifications, coaching decisions, user actions, enforcement actions, and optional recovery records. Character profiles reference model assets but are not joined to policy eligibility. Event history records state transitions with minimal data.

Delete/export should include relational history and asset files according to user choice; support a full local export with schema version and a deletion confirmation. Archive is distinct from delete. Migrations must be transactional and tested against previous released schemas. Backups are not automatic until specified; user-created backups need the same protection and deletion explanation.

## Data minimization and retention

- Do not persist screenshots or page text in MVP.
- Prefer app ID and coarse category over window title; if title is needed for a chosen feature, keep it in a short in-memory buffer and redact secrets/URLs where practical.
- Browser origin/URL collection is separate and opt-in; retention is not on by default pending a reviewed use case.
- Keep session summaries and user corrections for the user-selected history period; exact default and expiry job are open decisions.
- Provide deletion of a single session and all data. Delete logs/exports only when controlled by the app; explain external backups are outside direct control.
- Store provider credentials in an OS credential manager if feasible, never in this database in plaintext.
- Character binary assets are user content and may be large; separate asset storage, enforce bounds, and support export/removal.

