# Original User Request

## Initial Request — 2026-10-24T03:58:48Z

Build a production-grade foundation and MVP for Wowie, a goal-aware Linux desktop companion that manages focus commitments, tracks permitted desktop activity signals against active goals, applies rule-based accountability policies, and reviews session outcomes.

Working directory: /media/pirate/Shared/currently working/Wowie
Integrity mode: development

## Requirements

### R1. Core Domain Model and Local Persistence
Implement the core domain data models and SQLite persistence layer for Wowie. This includes user goals, time-bounded focus commitments/sessions, configurable break schedules, activity event audit logs, user corrections, and session reviews, with database migrations and schema initialization following `docs/DATA_MODEL.md`. Use Rust and SQLite.

### R2. Activity Observation and Privacy Safeguards
Implement the desktop activity observer abstraction and Linux observation adapter (handling active window title, process/application name, and idle detection), with a mock/testable provider for headless continuous integration. Ensure privacy safeguards: provide configurable exclusion lists, allow filtering private/sensitive window titles, support instant session pausing, and strictly enforce that observation only runs when a focus session is active.

### R3. Contextual Rule Policy and Decision Engine
Implement a deterministic, rules-based decision engine that compares active goal commitments with observed activity signals. The engine must support configurable grace periods, idle detection, snooze states, planned breaks, and user corrections. When activity conflicts with commitments, it must emit structured notification recommendations without permanently treating ambiguous sites or tutorials as hard violations.

### R4. CLI and Session Control Interface
Provide an executable command-line interface and/or daemon control commands to manage goals, initiate/stop/pause focus commitments, query live session status, manually submit activity corrections, and display post-session review summaries.

### R5. Automated Test Harness and End-to-End Simulation
Create an automated test suite including unit tests and an end-to-end integration test runner that simulates a full focus workflow: goal definition, starting a session, feeding simulated productive and distracted desktop activity events, verifying policy engine evaluation, processing a user correction, and validating that session history is persisted accurately.

## Acceptance Criteria

### Compilation & Code Quality
- [ ] `cargo check` and `cargo test` compile and pass with zero test failures across all packages/modules.
- [ ] `cargo clippy --all-targets -- -D warnings` passes with zero compiler/lint warnings.
- [ ] Code formatting adheres to `cargo fmt --check`.

### Functional Verification
- [ ] SQLite database file initializes cleanly with migrations, establishing tables for goals, sessions, activity logs, and reviews.
- [ ] Running the CLI can create a goal, start a focus session, report the remaining time, and finish or cancel the session.
- [ ] The policy engine correctly triggers advisory notifications after grace periods when conflicting activity is detected, pauses evaluation during breaks, and resumes cleanly.
- [ ] Submitting a user correction (e.g., classifying a tutorial as on-goal) updates the evaluation state immediately and is recorded in the session audit log.
- [ ] At session conclusion, a review summary is generated and persisted with session statistics, break duration, and logged events.

### End-to-End Integration
- [ ] An automated integration test executes a simulated end-to-end session without requiring GUI interactions, asserting state transitions from session start to completion.


## Follow-up — 2026-10-24T10:51:39Z

Implement the AI Coach and dialogue prompt engine for Wowie as a dedicated Rust workspace crate (`crates/wowie-coach`) that analyzes goals, session commitments, and observed desktop activity to produce structured, tone-aware coaching advice and dialogue without direct OS execution authority.

Working directory: `/media/pirate/Shared/currently working/Wowie`
Integrity mode: development

Reference material: `docs/AI_BEHAVIOR.md`, `docs/ACCOUNTABILITY_RULES.md`

## Requirements

### R1. Structured Coach Context & Response Contracts
Define strictly-typed data models for coaching inputs (active goal, next action, session commitment, permitted activity metadata, user correction history, tone preferences) and structured outputs (relevance classification, confidence score, uncertainty estimation, human-readable rationale, optional clarifying question, and an optional action candidate selected strictly from an allowed enum). Untrusted activity data (window titles, URLs) must be isolated with clear delimiter markers to resist prompt injection.

### R2. Pluggable Provider Abstraction
Provide an asynchronous LLM provider trait (`CoachProvider`) supporting offline/mock providers for zero-dependency testability, alongside an extensible client implementation for local or remote OpenAI-compatible endpoints. The engine must support graceful degradation: if a provider times out, fails, or returns malformed data, it must return a deterministic fallback without crashing.

### R3. Tone, Personality & Cooldown Policy
Support customizable tone and interaction parameters (e.g., directness, warmth, humor, neutral) that influence generated prompts without altering security boundaries or authority. Provide deterministic session cooldown tracking to prevent repetitive or excessive interventions during a focus session.

### R4. Security Boundaries & Action Allowlist
The coach engine must not possess capabilities to run shell commands, alter OS state, or bypass deterministic rules. Output parsing must strictly enforce the closed enum of allowable action recommendations and discard any unauthorized or unknown fields.

## Acceptance Criteria

### Compilation & Code Quality
- [ ] `cargo check -p wowie-coach --all-targets` passes with zero errors.
- [ ] `cargo clippy -p wowie-coach --all-targets` passes without warnings.
- [ ] Crate is properly registered in root `Cargo.toml` workspace members.

### Objective Automated Verification
- [ ] `cargo test -p wowie-coach` passes all unit and integration tests completely offline without external network or API keys.
- [ ] Unit tests verify prompt injection defense: inputs with adversarial instructions in window titles/URLs do not escape delimiters or corrupt parsed structured outputs.
- [ ] Schema validation tests verify that invalid JSON, missing fields, or unrecognized action types from the model trigger safe fallback errors.
- [ ] Cooldown tests verify that rapid successive intervention requests are throttled deterministically.
- [ ] Mock provider tests demonstrate end-to-end flow from input context to validated structured recommendation.


## Follow-up — 2026-10-25T04:18:58Z

The server has restarted and rate limits have reset. Please resume and finish the remaining milestones for wowie-coach (M4 gate completion, M5 CoachEngine facade, and M6 E2E hardening).

## Follow-up — 2026-10-25T20:21:57Z

Implement and verify GPU hardware acceleration (Vulkan/WGPU), expanded compositor support (GNOME Wayland and KDE Plasma Wayland), and local speech & audio interaction (Piper TTS) for the Wowie desktop AI companion.

Working directory: /media/pirate/Shared/currently working/Wowie
Integrity mode: development

## Requirements

### R1. GPU Hardware-Accelerated Rendering (Vulkan / WGPU)
Provide an optional hardware-accelerated rendering pipeline for high-polygon VRM/glTF character models in `wowie-overlay`. The renderer must detect GPU capability, support swapchain and shaders, and automatically fall back to the pure-Rust software rasterizer without crashing on systems without GPU drivers.

### R2. Expanded Wayland Compositor Support
Integrate focused application and window title observation for GNOME Wayland (via Mutter D-Bus / XDG Desktop Portal) and KDE Plasma Wayland (via KWin D-Bus/Scripting) in `wowie-observer`, respecting active focus session privacy and title sanitization boundaries.

### R3. Speech & Audio Interaction
Provide local text-to-speech (TTS) audio synthesis (supporting Piper neural TTS, `espeak-ng`, and `spd-say`) in `wowie-avatar` to announce session starts, interventions, breaks, and celebrations with user-configurable voice preferences and complete offline operation.

## Acceptance Criteria

### 1. Functional Verification
- [ ] GPU rendering backend abstraction initializes and renders or falls back to CPU software rasterizer gracefully on all environments.
- [ ] GNOME and KDE platform adapters accurately extract focused application and window title or report unavailable without panicking.
- [ ] Speech synthesizer executes configured TTS binaries asynchronously without blocking audio or main event loop.

### 2. Code Quality & Security Invariants
- [ ] `#![forbid(unsafe_code)]` remains strictly enforced across all workspace crates.
- [ ] Zero OS execution authority for AI: voice synthesis and compositor tracking are deterministic and policy-governed.
- [ ] All workspace tests (`cargo test --workspace`) pass 100% offline.
- [ ] Zero Clippy warnings (`cargo clippy --workspace --all-targets -- -D warnings`).
- [ ] Zero formatting diffs (`cargo fmt --check`).
