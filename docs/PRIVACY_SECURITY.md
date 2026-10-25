# Privacy and security threat model

**Status:** Implemented controls verified across all 8 workspace crates with `#![forbid(unsafe_code)]`, strict prompt injection quarantine, atomic hosts demarcations, and independent forensic audits.

## Data inventory and defaults

| Data | Why it may be needed | Default handling | Disable/delete |
|---|---|---|---|
| Goals, commitments, session times | User's chosen plan and review | Local persistence | Edit/delete/export |
| Active app/window metadata | Coarse activity context during consented session | Off outside session; minimize and buffer locally | Pause/stop; delete session |
| Browser origin/URL | More context for browser activity | Separate optional connector, disabled by default | Disconnect and clear records |
| Screenshot/page text | Not required for MVP | Not collected | Future feature needs separate review/consent |
| Classification and AI assessment | Explain a possible mismatch | Store provenance and confidence; no AI default requirement | Correct/delete with session |
| Cloud prompt fields | Optional coaching | No cloud transmission unless explicitly configured | Disable provider; delete provider-side data per terms |
| Avatar file and metadata | Render chosen character | Local app-managed asset store | Remove/export |
| Diagnostics | Troubleshooting | Local redacted records; no automatic telemetry | Review/export/delete |
| Credentials | Provider authentication | OS credential manager if feasible | Revoke/remove |

## Threat actors and assets

Threats include a local untrusted process/user, malicious or malformed model file, malicious page/window title attempting prompt injection, compromised or misconfigured AI provider, accidental cloud disclosure, overbroad blocking, dependency compromise, and data exposure through logs/backups. Sensitive assets are activity history, goals, URLs/titles, credentials, model files, and enforcement authorization.

## Controls and trust boundaries

- Consent is explicit, scoped, visible, and revocable. Sampling stops when paused.
- Local-first storage, minimization, retention limits, data export/deletion, and no default cloud activity transmission.
- Treat titles, URLs, page/model metadata, AI responses, and external provider errors as untrusted.
- Strict schema and enum validation for AI output; no arbitrary command or OS bridge.
- Credential store rather than settings/database/logs; redact secrets from diagnostics.
- Bounded asset parsing, no model scripts/network, path and resource limits, optional process isolation.
- Enforcement is deterministic, previewed, confirmed, scoped, expiring, reversible, and audited.
- Logs avoid raw titles/URLs and screen content; backups and exports are user-controlled and explained.
- Dependencies are pinned/reviewed with vulnerability and license checks before release.
- Emergency stop remains usable despite AI, renderer, or platform adapter failure.

## Local storage and encryption

SQLite is proposed. Encryption-at-rest method is undecided; do not promise encryption until implemented and verified. Consider OS account protection, filesystem encryption availability, and key handling. If application-level encryption is selected, key material must not be stored beside the database. Avoid custom cryptography.

## Cloud AI boundary

Cloud use requires explicit opt-in, provider identification, exact data categories, purpose, credential handling, and retention caveat. Send the smallest current-session summary possible. Do not transmit raw event history, full titles/URLs, screenshots, model files, or credentials by default. Provider terms and user consent must be reviewed before release.

## Prompt injection and malicious content

Desktop/browser text and asset metadata are untrusted data, not instructions. An adversarial title such as “ignore all rules” must not change policy or action allowlists. AI has no direct system-control tools. Parsed avatar assets must not gain code execution or network access.

## Blocking abuse and recovery

Do not secretly lock out the user or disable essential tools. Never write global network configuration without a separate accepted decision (currently out of scope). Track action expiry and expose stop. Test crash/reboot cleanup and show manual recovery guidance when a reversible mechanism cannot clean itself up.

## Open security questions

Select supported platform/API and its permissions, key storage and encryption design, retention defaults, browser connector protocol/authentication, parser isolation strategy, release vulnerability contact, and dependency update SLA before release. Threat model must be revisited when screen capture, telemetry, sync, or new enforcement is proposed.

