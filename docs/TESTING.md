# Testing strategy

This strategy is proposed; no tests or test framework exist yet. Use requirement IDs from [Product requirements](PRODUCT_REQUIREMENTS.md) and acceptance cases from [TODO](TODO.md).

## Test layers

- **Unit:** goal/session transitions, time math, normalizer, confidence combination, cooldowns, consent, schema validation, retention, and migration logic.
- **Integration:** repository transactions, app orchestrator, provider adapters, notification bridge, browser connector, asset registry, enforcement adapter.
- **Activity classification:** labeled deterministic fixtures for tutorials, documentation, unrelated video, research, switching, idle, unknown, misleading titles, and user correction.
- **AI decision:** fixed provider fixtures; same request must yield stable parsed result. Cover refusal, timeout, malformed/oversized output, injection text, missing provider, and no-AI path.
- **Policy/enforcement:** table-driven state combinations and fake clock; action without commitment, consent, supported target, or explicit confirmation must be denied.
- **Avatar import:** malformed containers, path traversal, external references, unsupported required extensions, huge textures/meshes, invalid rigs, missing animations, license metadata, and fallback.
- **UI/user-flow:** onboarding skip paths, consent, session lifecycle, correction, break/snooze, review, pause, emergency stop, deletion.
- **Platform:** named compositor/session/version matrix; permission denial, unavailable APIs, suspend/resume, notification behavior, app crash, and cleanup.
- **Performance/resource:** measure idle and active CPU/memory, sampling overhead, renderer frame/resource use, AI latency, and database growth on declared reference devices. Do not invent budgets; approve them from results.
- **Privacy/security:** inspect network egress, logs, data deletion, credential redaction, permission scope, prompt injection, dependency inventory, and asset parser limits.
- **End-to-end acceptance:** run requirements FR-001–018's MVP scope with fixtures, then supported real desktop adapter; distinguish notification vertical slice from avatar follow-on milestone.

## Reliability criteria

Before MVP release, policy outcomes are deterministic for fixed inputs; AI is never required; denied/unavailable integrations cause no action; monitoring stop is verified; all action paths have expiry/undo; data export/deletion is verified; target platform matrix is explicit; and false-positive/missed-event behavior is measured and reviewed. Product owner approves acceptable rates because no evidence currently exists to set them.

## Key fault scenarios

- False positive: user is watching a relevant lesson. Prompt must offer correction, correction suppresses repeats, and no block is automatic.
- Missed distraction: title/URL looks relevant but is not. User can optionally correct; product must not claim perfect detection.
- AI timeout: session and policy continue with rules only.
- Integration unavailable: monitoring state reports unavailable; no stale event is treated as current.
- Enforcement cleanup failure: action status remains visible with a manual recovery path.
- Renderer crash: notification and session controls remain available.

