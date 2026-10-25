# Product requirements

**Status:** Implemented & Verified baseline. All core functional requirements (FR-001 through FR-018) are operational across the 8 workspace crates and verified via automated test suites and live Linux execution.

## Users and problems

Primary users are people pursuing self-directed learning, creative work, or personal projects who want help converting broad intent into manageable commitments. They may lose time to unrelated activity, struggle to resume after a lapse, or find generic timers and strict blockers poorly matched to their goals. Users may also want a chosen character and tone, but must be able to use Wowie without one.

## Product goals

- Turn an ambition into a concrete next step and time-bounded commitment.
- Give goal-aware support based on limited, disclosed evidence and user corrections.
- Make interventions adjustable, respectful, rate-limited, and recoverable.
- Keep core focus and review flows usable without AI or avatar rendering.
- Make local-first handling, monitoring consent, and optional integrations understandable.
- Support user-selected character assets through a validated, extensible path.

## Non-goals for initial release

- Replacing a therapist, teacher, manager, or productivity suite.
- Judging whether a user is morally productive or treating leisure as inherently bad.
- Continuous screenshot analysis, keylogging, microphone recording, or hidden surveillance.
- AI-issued OS commands, forced lockout, severe punishment, or irreversible enforcement.
- Cloud account synchronization, multi-user service, mandatory subscription, or microservice deployment.
- Universal Linux compositor support or first-release parity on Windows/macOS.
- A bundled fixed mascot as the only character option.

## Core use cases

1. Define an ambition, a goal, and a small next action.
2. Start, pause, resume, and finish a focus session.
3. Enable visible activity metadata monitoring for a session.
4. Receive a cautious mismatch prompt and explain/correct context.
5. Take a planned break, snooze a reminder, or request a bounded temporary block.
6. Review session history and recover constructively after a missed commitment.
7. Configure privacy, AI, personality, notifications, and optional character.
8. Import a model only after compatibility, resource, and license information is surfaced.

## Functional requirements

| ID | Requirement | MVP |
|---|---|---|
| FR-001 | Create, edit, archive, and delete user-authored ambitions and goals, each with an optional concrete next action. | Yes |
| FR-002 | Create commitments with duration, allowed break policy, and associated goal. | Yes |
| FR-003 | Start, pause, resume, end, and recover an interrupted focus session. | Yes |
| FR-004 | Show monitoring state and permission scope; provide immediate pause and disable controls. | Yes |
| FR-005 | Observe only explicitly configured desktop metadata through a named platform adapter; unavailable signals are visible. | Yes |
| FR-006 | Normalize observations into time-bounded activity segments and distinguish idle, switching, break, and unknown states. | Yes |
| FR-007 | Assess possible goal mismatch using user rules and optional AI; preserve uncertainty and provenance. | Yes |
| FR-008 | Require deterministic policy evaluation before any intervention; entertainment alone is never a violation. | Yes |
| FR-009 | Deliver an intervention through desktop notification in the initial vertical slice; character rendering is optional for MVP. | Yes |
| FR-010 | Let users resume, correct classification, mark a planned break, snooze, or dismiss an intervention. | Yes |
| FR-011 | Permit a user-requested, scoped, expiring website block only when an explicitly configured adapter is available. | Later MVP increment |
| FR-012 | Record session outcome, relevant activity summaries, corrections, and authorized actions for review. | Yes |
| FR-013 | Export and delete user data and define retention defaults. | Yes |
| FR-014 | Configure local AI, cloud AI, or no AI; cloud transmission is opt-in and disclosed. | Yes (no-AI path required) |
| FR-015 | Import and configure supported custom model assets with validation, license disclosure, and fallback. | Follow-on milestone |
| FR-016 | Configure name, tone, reminders, thresholds, resources, and monitoring scope. | Yes |
| FR-017 | Run the core session and policy flow when AI provider, avatar renderer, or optional platform integration is unavailable. | Yes |
| FR-018 | Provide user-visible stop, pause, recovery, and undo paths for temporary enforcement. | Yes |

## Non-functional requirements

| ID | Requirement | MVP evidence |
|---|---|---|
| NFR-001 | Minimize data collection and default to local processing; raw screen content is off and absent from MVP. | Data-flow review and tests |
| NFR-002 | Monitoring is visible, permission-based, session-scoped by default, and easy to pause. | User-flow acceptance and platform tests |
| NFR-003 | Policy decisions are deterministic, auditable, and testable without AI or OS integrations. | Fixture-based policy suite |
| NFR-004 | Failures degrade safely: no AI means rule-based/basic session; no metadata means no inference; no renderer means notification. | Failure-path tests |
| NFR-005 | Resource limits are configurable and observable; target budgets must be measured before acceptance. | Profiling report; numeric budgets to be approved |
| NFR-006 | Persisted data supports schema migrations, export, and deletion. | Migration and data lifecycle tests |
| NFR-007 | User-authorized enforcement is temporary, scoped, reversible, logged, and overridable. | Enforcement tests and review |
| NFR-008 | Linux support is stated by tested desktop/session integration, not generalized from one environment. | Compatibility matrix |
| NFR-009 | Importers enforce file, geometry, texture, and parser bounds and never execute model-supplied code. | Malformed-asset test corpus |
| NFR-010 | AI output is treated as untrusted, does not directly control OS behavior, and communicates uncertainty. | Prompt-injection and contract tests |
| NFR-011 | The product remains usable offline and has no mandatory subscription or cloud dependency. | Offline acceptance test |
| NFR-012 | All user-facing and technical claims distinguish proposed, supported, and verified behavior. | Release documentation review |

## Acceptance criteria for major MVP capabilities

- **Goals and commitments (FR-001–003):** a user can create a goal with a next action, start a timed session, pause/resume it, finish early or at expiry, and recover a session after restart without silently extending it.
- **Monitoring (FR-004–006):** consent explains exact signal and boundaries; an always-visible state indicator reflects active monitoring; pause takes effect promptly; unavailable integration yields an explicit “not observing” state; no unsupported inference is made from title-only data.
- **Assessment and intervention (FR-007–010):** fixed fixtures produce stable classifications and policy outcomes; uncertain/educational/ambiguous content does not trigger an automatic violation; intervention identifies its basis and offers correction, break, snooze, dismiss, and resume actions.
- **AI and offline (FR-014, FR-017):** no-AI mode completes a session; provider timeout, malformed response, and missing credentials degrade to safe deterministic behavior; cloud mode requires a specific opt-in and shows what fields are transmitted.
- **History and controls (FR-012–013, FR-018):** user can review session events, export and delete local history, pause monitoring, stop a temporary block, and see action expiry.
- **Blocking increment (FR-011):** requested block is previewed with target, scope, expiry, consequences, and stop control; no action occurs until user confirms; adapter failure leaves user access intact and explains failure.
- **Character follow-on (FR-015):** an import report lists supported features, unsupported features, metadata/license declaration, resource use, and safe fallback; removing the asset deletes app-managed copies and references.

## Risks, assumptions, dependencies, unresolved questions

- Linux window/activity APIs differ by compositor and protocol; reliable active-window metadata may require desktop-specific integration or explicit user setup.
- Browser URL integration likely requires an optional extension with additional trust, packaging, and maintenance work. URL is not equivalent to page meaning.
- Content relevance is inherently ambiguous; title/URL heuristics and AI can both err. How much correction friction users accept is unknown.
- User-imported models vary widely in format features, licensing, size, complexity, and malicious-file risk.
- Godot, Qt, Tauri/WebView, GTK, and renderer combinations have different packaging, overlay, idle-resource, and license trade-offs.
- Local model quality and hardware requirements vary. Cloud AI introduces privacy, cost, availability, and policy questions.
- Reliable website blocking on Linux may require per-browser extension or privileged/network configuration; no universal mechanism is assumed.
- Numeric CPU/memory budgets, exact supported desktop environments, default sampling interval, and retention period require measurement and owner approval.

Open questions are tracked in [DECISIONS.md](DECISIONS.md) and [TODO.md](TODO.md). Global product rules live only in [ACCOUNTABILITY_RULES.md](ACCOUNTABILITY_RULES.md).

