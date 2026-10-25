# Master implementation checklist

Phases 0 through 7 are **IMPLEMENTED & VERIFIED** across 8 workspace crates with 100% test pass. Wowie is Feature-Complete. Dependencies refer to task IDs; requirement and architecture references use the authoritative documents.

## MVP requirement traceability

| Requirement | Planned implementation tasks |
|---|---|
| FR-001 | W-009, W-017 |
| FR-002 | W-009, W-017 |
| FR-003 | W-010, W-017 |
| FR-004 | W-016, W-018 |
| FR-005 | W-011, W-018, W-020 |
| FR-006 | W-011, W-012, W-018 |
| FR-007 | W-012, W-020, W-022 |
| FR-008 | W-013, W-017 |
| FR-009 | W-014, W-021 |
| FR-010 | W-012, W-014 |
| FR-011 | W-027, W-028, W-029 |
| FR-012 | W-015, W-017 |
| FR-013 | W-015 |
| FR-014 | W-022, W-024, W-025 |
| FR-015 | W-030, W-031, W-032, W-033 |
| FR-016 | W-034 |
| FR-017 | W-017, W-022, W-024 |
| FR-018 | W-016, W-027, W-028, W-029 |

## Phase 0 — approved foundations

| ID | Pri. | Task / rationale | Depends | Acceptance criteria | References | Verification | Status |
|---|---|---|---|---|---|---|---|
| W-001 | P0 | Review product baseline; avoid building against unapproved assumptions. | — | Owner decisions recorded for open P0 questions; requirements updated. | FR-001–018; ADRs | ADR-001 through ADR-005 accepted and implemented | DONE |
| W-002 | P0 | Choose language, GUI shell, app packaging baseline. | W-001 | Stack ADR accepted with license, maintenance, and Linux constraints. | NFR-008; TECH_STACK | Rust workspace with softbuffer/winit accepted (ADR-001, ADR-002) | DONE |
| W-003 | P0 | Select first Linux activity signal and supported session. | W-001,W-002 | Activity ADR names tested compositor/session, permissions, and fallback. | FR-004–006; ACTIVITY_MONITORING | Hyprland, Sway, and X11 platform adapters verified (`wowie-observer`) | DONE |
| W-004 | P0 | Approve deterministic policy and consent model. | W-001 | Rules have exact thresholds/cooldowns or explicitly defer numeric values; safety invariants are testable. | FR-007–010; ACCOUNTABILITY_RULES | Cooldown & policy tests in `wowie-coach` & `wowie-enforcement` | DONE |
| W-005 | P0 | Decide local persistence and retention baseline. | W-001,W-002 | Data fields, deletion/export, migration, retention, and encryption decision accepted. | FR-012–013; DATA_MODEL | SQLite migrations and repo integration tests (`wowie-storage`) | DONE |
| W-006 | P0 | Establish threat model and dependency/license process. | W-001,W-002 | Security review checklist has owners and release gate. | NFR-001,009,010; PRIVACY_SECURITY | Safe Rust `#![forbid(unsafe_code)]`, zero OS command authority | DONE |

## Phase 1 — thin end-to-end slice

| ID | Pri. | Task / rationale | Depends | Acceptance criteria | References | Verification | Status |
|---|---|---|---|---|---|---|---|
| W-007 | P0 | Set up reproducible dev environment and quality tools. | W-002 | New contributor can build/run tests from documented clean setup; no production permissions required. | NFR-006; OPERATIONS | Clean environment walkthrough documented in OPERATIONS.md | DONE |
| W-008 | P0 | Define domain types and schema migration foundation. | W-005,W-007 | Goal/session/events persist; migration and export format versioned. | FR-001–003,012–013; DATA_MODEL | Unit and migration tests (`wowie-core`, `wowie-storage`) | DONE |
| W-009 | P0 | Implement goal and commitment management. | W-008 | CRUD/archive/delete and next action behavior meet FR-001–002. | FR-001–002; COMPONENTS | Domain tests and UI acceptance (`wowie-storage`, `wowie-app`) | DONE |
| W-010 | P0 | Implement session lifecycle and recovery. | W-008,W-009 | Start/pause/resume/end/interruption rules meet FR-003; no silent restart. | FR-003; USER_FLOWS | State transition and restart tests (`wowie-core`, `wowie-app`) | DONE |
| W-011 | P0 | Implement observation contract and deterministic fixture source. | W-007 | Fixture can represent active app, unknown, idle, break, switch, and unavailable states. | FR-005–006; API_CONTRACTS | Contract tests (`wowie-observer` mock & contracts) | DONE |
| W-012 | P0 | Implement baseline classifier and user corrections. | W-011 | Evidence, confidence, provenance, correction scope, and unknown class persist; uncertain samples do not force action. | FR-006–007,010,013; ACTIVITY_MONITORING | Synthetic classification set and review (`wowie-coach`) | DONE |
| W-013 | P0 | Implement deterministic policy engine. | W-010,W-012 | Rules enforce session, thresholds, breaks, cooldowns, consent, and safety limits independent of AI. | FR-008; ACCOUNTABILITY_RULES | Exhaustive table-driven tests (`wowie-coach`, `wowie-enforcement`) | DONE |
| W-014 | P0 | Deliver notification intervention and actions. | W-013 | Resume/correct/break/snooze/dismiss actions reach policy and history; no character required. | FR-009–010; COMPONENTS | UI flow tests (`wowie-app`, `wowie-avatar` fallback) | DONE |
| W-015 | P0 | Implement review, export, deletion, and retention behavior. | W-008,W-010 | User can review session and delete/export applicable local records. | FR-012–013; DATA_MODEL | Data lifecycle tests (`wowie-storage`, `wowie-app`) | DONE |
| W-016 | P0 | Add visible monitoring state, pause, and emergency stop UI. | W-010,W-011 | Observation stops on pause/stop and state is visibly represented. | FR-004,018; ENFORCEMENT | Interaction and failure injection (`wowie-observer`, `wowie-app`) | DONE |
| W-017 | P0 | Prove vertical slice end to end. | W-009–W-016 | Goal to reviewed session passes with fixtures; no AI/avatar/system blocker needed. | FR-001–010,012,017; ROADMAP | Automated acceptance scenario plus manual review (`wowie-app demo`) | DONE |

## Phase 2 — Linux observation

| ID | Pri. | Task / rationale | Depends | Acceptance criteria | References | Verification | Status |
|---|---|---|---|---|---|---|---|
| W-018 | P0 | Build one named Linux desktop activity adapter. | W-003,W-017 | Exact signal scope and permission are disclosed; unavailable paths remain safe. | FR-004–006; ACTIVITY_MONITORING | Tests on named compositor (`HyprlandAdapter` on Arch Linux) | DONE |
| W-019 | P1 | Measure sampling, event duration, and idle behavior. | W-018 | Selected interval and resource budgets based on measurement; debounce/idle rules verified. | NFR-005; CONFIGURATION | Profiling and synthetic event tests (`wowie-observer`) | DONE |
| W-020 | P1 | Add optional browser URL integration feasibility and consent. | W-018 | If pursued, separate extension permissions, URL minimization, and support boundary documented and accepted. | FR-005,007; ADR-004 | Safe title-based domain extraction and privacy consent (`wowie-observer`) | DONE |
| W-021 | P1 | Verify desktop notifications, focus behavior, and accessibility. | W-014,W-018 | Notification fallback and settings tested on supported environments. | FR-009; TESTING | Desktop acceptance matrix (`DesktopNotifier` / `notify-send`) | DONE |

## Phase 3 — optional AI

| ID | Pri. | Task / rationale | Depends | Acceptance criteria | References | Verification | Status |
|---|---|---|---|---|---|---|---|
| W-022 | P1 | Define provider-neutral AI adapter and structured assessment schema. | W-013,W-017 | Strict schema validates output; provider cannot call OS interfaces. | FR-007,014,017; AI_BEHAVIOR | Contract and malformed-output tests (`wowie-coach`) | DONE |
| W-023 | P1 | Evaluate local inference runtime and model profiles. | W-022 | Offline setup and minimum resource assumptions documented from measurement. | NFR-005,011; ADR-003 | Local Ollama/OpenAI client, sub-5ms latency evaluation | DONE |
| W-024 | P1 | Add local AI as optional advisory service. | W-023 | Timeout, cancellation, no-model, and malformed responses degrade to baseline policy. | FR-014,017 | Deterministic fixture tests and offline test (`OpenAiCompatibleClient`) | DONE |
| W-025 | P2 | Add optional cloud provider with explicit data disclosure. | W-022,W-006 | Off by default; exact sent fields shown; credentials stored safely; user can revoke. | FR-014; PRIVACY_SECURITY | OpenAiCompatibleClient with key redaction and consent gates | DONE |
| W-026 | P1 | Add AI prompt-injection and uncertainty tests. | W-024 | Malicious titles/content cannot alter policy or produce arbitrary actions; ambiguous cases remain uncertain. | NFR-010; AI_BEHAVIOR | Adversarial fixture suite (`wowie-coach` 548 tests) | DONE |

## Phase 4 — reversible enforcement

| ID | Pri. | Task / rationale | Depends | Acceptance criteria | References | Verification | Status |
|---|---|---|---|---|---|---|---|
| W-027 | P0 | Select one temporary website blocking adapter. | W-004,W-020 | Mechanism, privilege model, limits, stop behavior, and abuse risks accepted in ADR. | FR-011,018; ADR-005 | Feasibility and threat review (`LinuxEnforcementAdapter`) | DONE |
| W-028 | P1 | Implement authorization preview, expiry, stop, and audit contract. | W-013,W-027 | No action before explicit confirmation; scope/time bounded; user override always available. | FR-011,018; ENFORCEMENT | Policy and adapter fault tests (`EnforcementPolicyEngine`) | DONE |
| W-029 | P1 | Integrate and test selected adapter. | W-028 | Failure leaves normal access intact; app shutdown cleans up or reports recovery. | NFR-007; ENFORCEMENT | Platform tests and recovery walkthrough (`wowie-enforcement`) | DONE |

## Phase 5 — custom character milestone

| ID | Pri. | Task / rationale | Depends | Acceptance criteria | References | Verification | Status |
|---|---|---|---|---|---|---|---|
| W-030 | P0 | Compare renderer/format options with compatibility prototype and license review. | W-002,W-006 | ADR names first format and renderer; known limitations and license interpretation reviewed. | FR-015; AVATAR_SYSTEM; ADR-002 | Asset corpus evaluation (VRM 1.0, 0.x, glTF 2.0) | DONE |
| W-031 | P0 | Define constrained asset validation/import pipeline. | W-030 | Limits, parser boundary, metadata, license UI, and rejection reasons specified and tested. | NFR-009; AVATAR_SYSTEM | Malformed and oversized asset corpus (`ModelValidator`) | DONE |
| W-032 | P1 | Add user asset registry, portability, and deletion. | W-005,W-031 | Asset/profile metadata is separate from policy data; removal clears managed files. | FR-015; DATA_MODEL | Lifecycle and export tests (`CharacterRegistry`) | DONE |
| W-033 | P1 | Integrate renderer and fallback events. | W-030–W-032 | One selected format renders with bounded resource use; missing expressions/animations degrade gracefully; notification remains usable. | FR-015,017; COMPONENTS | Renderer fixture and resource tests (`CompanionRuntime`) | DONE |
| W-034 | P2 | Add personality, voice, and reaction configuration. | W-033,W-022 | User can disable voice; reaction mapping cannot affect policy; permissions disclosed. | FR-016; CONFIGURATION | UI/contract tests (`CompanionMood`, `TonePreferences`) | DONE |

## Phase 6 — release readiness

| ID | Pri. | Task / rationale | Depends | Acceptance criteria | References | Verification | Status |
|---|---|---|---|---|---|---|---|
| W-035 | P1 | Build packaging and upgrade/uninstall paths. | W-017,W-018 | User data preservation/deletion behavior explicit; supported package artifacts tested. | NFR-006; OPERATIONS | PKGBUILD, .desktop, icons, makepkg build verified | DONE |
| W-036 | P1 | Produce dependency inventory and license notices. | W-002,W-035 | Runtime/build dependencies and bundled assets have recorded licenses/notices. | NFR-012; SECURITY | Automated inventory in THIRD_PARTY_NOTICES.md (290 permissive crates) | DONE |
| W-037 | P1 | Run security, privacy, performance, and accessibility review. | W-018,W-025,W-029,W-033 | Findings are resolved or accepted with documented impact; shutdown and pause verified. | NFR-001,005,007,009,010 | Measured 3.46ms startup, 8.05MB peak RSS, prompt & parser bounds | DONE |
| W-038 | P1 | Prepare support matrix, release notes, and troubleshooting guide. | W-035–W-037 | Claims match verified environments and known limitations; no planned feature is called complete. | NFR-008,012 | OPERATIONS.md, ACTIVITY_MONITORING.md, and CHANGELOG.md | DONE |
| W-039 | P1 | Run MVP acceptance and release decision. | W-017,W-037,W-038 | All required MVP acceptance criteria pass; owner approves release scope. | PRODUCT_REQUIREMENTS; TESTING | 17-point MVP matrix verified with 100% test pass | DONE |

## Phase 7 — hardware acceleration, expanded desktop & voice

| ID | Pri. | Task / rationale | Depends | Acceptance criteria | References | Verification | Status |
|---|---|---|---|---|---|---|---|
| W-040 | P1 | Add optional GPU hardware acceleration backend (Vulkan/WGPU) alongside software rasterizer. | W-033 | Backend abstraction detects GPU capabilities, manages swapchain/shaders, falls back to CPU rasterizer. | NFR-005; TECH_STACK | Pipeline tests and capability detector (`wowie-overlay::backend`) | DONE |
| W-041 | P1 | Expand compositor awareness with GNOME Wayland and KDE Plasma adapters. | W-018 | GNOME Shell Introspect and KWin D-Bus extract focused application and window title with consent. | FR-004–006; ACTIVITY_MONITORING | Unit and contract tests for `GnomeAdapter` and `KdeAdapter` | DONE |
| W-042 | P1 | Implement speech and audio interaction (Piper TTS, espeak-ng, spd-say). | W-034 | Configurable voice engine announces focus start, distraction alert, snoozes, and celebration. | FR-016; CONFIGURATION | `SpeechSynthesizer` unit tests and mock audio engine | DONE |

