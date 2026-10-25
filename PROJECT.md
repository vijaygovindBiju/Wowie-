# Project: Wowie Desktop AI Companion

## Architecture
- **Multi-Crate Workspace**:
  1. `crates/wowie-core`: Domain models, strictly typed IDs, invariants, and shared state machines (goals, commitments, sessions, observations, corrections, reviews).
  2. `crates/wowie-storage`: SQLite persistence layer, transaction pool, idempotent migrations (`001_initial.sql`), checksum tampering detection, active session singleton constraint, and repository implementations.
  3. `crates/wowie-coach`: Contextual AI coaching engine, delimiter isolation (`<untrusted_activity_evidence>`), adversarial injection resistance, closed action allowlist, tone formatting, deterministic cooldowns, pluggable `CoachProvider` (`MockCoachProvider`, `OpenAiCompatibleClient`), and deterministic fallbacks.
  4. `crates/wowie-observer`: Linux desktop activity monitoring, platform auto-detection (`HyprlandAdapter` using `hyprctl`, `X11Adapter`, `MockObserver`), user consent enforcement, window title redaction, origin extraction, idle state detection, and pause/stop controls.
  5. `crates/wowie-avatar`: 3D character (VRM/glTF) security bounds validator (`ModelValidator`), URI traversal/external request rejection, metadata extraction (expressions, license, authors), `CharacterRegistry` for safe imports/deletions, real binary 3D geometry decoding (`load_3d_model`), bounding box normalization, and companion runtime with mood mapping.
  6. `crates/wowie-overlay`: Dedicated desktop companion overlay. Pure Rust 3D rasterizer with Z-buffering, perspective camera with orbit/zoom, directional + ambient Gouraud shading, mood animation transforms (idle, focused, distraction alert, snoozed, celebration), HUD status pill, and transparent Wayland/Hyprland windowing via `winit` 0.30 and `softbuffer` 0.4.
  7. `crates/wowie-enforcement`: Reversible distraction enforcement, hardcoded protection for essential developer/system tools (`cargo`, `rustc`, `git`, `code`, `alacritty`, `docs.rs`, etc.), single-use 120s confirmation tokens, `LinuxEnforcementAdapter` with demarcated section editing, non-destructive crash recovery, pre-mutation backup (`.wowie_backup`), and `EnforcementManager` with automatic cleanup on drop/stop.
  8. `crates/wowie-app`: Application coordinator, state management, CLI runner (`wowie run`, `wowie demo`, `wowie status`, `wowie character [list|select|3d-info|import]`, `wowie overlay`), terminal UI renderer (8 full screens), overlay event bridge, and end-to-end integration workflows.

- **Zero OS Authority for AI**: The AI coach crate (`wowie-coach`), overlay, and avatar crates have zero OS execution privileges. All system and policy decisions are deterministic.
- **Defense in Depth**: Delimiter isolation, entity escaping, length bounding, schema validation, essential tool immunities, section demarcation, and safe Rust bounds prevent unintended lockouts, hosts destruction, and injection attacks.
- **Verified Environment**: Arch Linux (`Linux 7.2.7-arch1-1`) under Hyprland with Wayland native integration.

## Feature Inventory
| # | Feature | Description | Crate | Status |
|---|---------|-------------|-------|--------|
| 1 | Core Domain Models | Ambition, Goal, Commitment, FocusSession, Observation, UserCorrection, SessionReview. | `wowie-core` | DONE |
| 2 | SQLite Storage & Migrations | Migration runner with checksum verification, atomic transactions, active session singleton constraint. | `wowie-storage` | DONE |
| 3 | AI Coaching Engine | Structured context/response, PromptSanitizer, CooldownTracker, Mock & OpenAI-compatible providers. | `wowie-coach` | DONE |
| 4 | Linux Activity Observer | Compositor adapter for Hyprland (`hyprctl activewindow -j`), title redaction, consent checks. | `wowie-observer` | DONE |
| 5 | 3D Character Runtime & Security | Safe VRM/glTF validation (50MB cap, no external URIs/scripts), metadata extraction, binary geometry extraction, mood mapping. | `wowie-avatar` | DONE |
| 6 | 3D Desktop Companion Overlay | Pure Rust 3D rasterizer (Z-buffer, Gouraud lighting, camera orbit/zoom), Wayland transparent window, Hyprland rules, mood animations. | `wowie-overlay` | DONE |
| 7 | Distraction Enforcement | User-authorized temporary blocking, essential tool immunities, demarcated sections, crash recovery, single-use confirmation tokens. | `wowie-enforcement` | DONE |
| 8 | Full UI & CLI Runner | 8-screen terminal UI renderer, CLI runner (`run`, `demo`, `status`, `character`, `overlay`), coordinator workflow. | `wowie-app` | DONE |

## Workspace Structure
```
.
├── Cargo.toml
├── crates/
│   ├── wowie-core/
│   │   ├── src/lib.rs (Domain models, types, IDs)
│   │   └── tests/
│   ├── wowie-storage/
│   │   ├── src/ (Pool, migrations, repositories)
│   │   └── tests/ (Adversarial storage, repo integration)
│   ├── wowie-coach/
│   │   ├── src/ (Sanitizer, builder, cooldown, providers, engine)
│   │   └── tests/ (Adversarial schema/encoding, e2e tiers 1-5)
│   ├── wowie-observer/
│   │   ├── src/ (PlatformObserver, HyprlandAdapter, X11Adapter, MockObserver, ActivityObserver)
│   │   └── tests/ (Observer unit & live Hyprland tests)
│   ├── wowie-avatar/
│   │   ├── src/ (ModelValidator, VRM/glTF parser, binary geometry loader, CharacterRegistry, CompanionRuntime)
│   │   └── tests/ (Security bounds, traversal, metadata, 3D model parsing)
│   ├── wowie-overlay/
│   │   ├── src/ (Math, 3D Rasterizer, Z-buffer, Lighting, Winit/Softbuffer Wayland window, Bridge, main.rs)
│   │   └── tests/ (Camera, rasterizer, depth testing, mood animations, bridge)
│   ├── wowie-enforcement/
│   │   ├── src/ (PolicyEngine, LinuxEnforcementAdapter with demarcated sections/crash recovery, EnforcementManager)
│   │   └── tests/ (Essential tools, token single-use, hosts preservation, crash recovery, backup)
│   └── wowie-app/
│       ├── src/ (Coordinator, AppState, AppSettings, UiRenderer, CliRunner, OverlayBridge, main.rs)
│       └── tests/ (App integration, 8-screen UI render, e2e lifecycle)
└── docs/
```

## Verification Summary
- `cargo fmt --check`: Clean (0 errors).
- `cargo clippy --workspace --all-targets -- -D warnings`: Clean (0 warnings).
- `cargo test --workspace`: 100% pass across all 8 workspace crates (118 tests).
- Live graphical verification: `cargo run -p wowie-overlay` verified live on Wayland / Hyprland session; screenshot captured and validated at `/tmp/wowie_overlay_live.png`.
- Live CLI verification: `cargo run -p wowie-app -- demo` verified end-to-end on Arch Linux / Hyprland.
