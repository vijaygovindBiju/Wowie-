# Changelog

All notable changes to the Wowie project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.0] - 2026-10-25

### Added
- **Core Domain Engine (`wowie-core`):**
  - Goal, Ambition, Actionable Commitment, Focus Session, Observation, and Session Review domain models.
  - Strict session state machine (Pending -> Active <-> Paused <-> Break -> Completed/Abandoned).
  - Explicit user corrections and intervention action models.
- **Local SQLite Storage (`wowie-storage`):**
  - ACID transactions, WAL mode, foreign keys, and atomic schema migrations with SHA-256 tamper detection.
  - Active session singleton database constraint preventing duplicate active sessions.
  - Cascade deletion rules and historical retention cleanup.
- **Contextual AI Coaching Engine (`wowie-coach`):**
  - Delimiter quarantine (`<untrusted_activity_evidence>`, `</untrusted_activity_evidence>`).
  - BiDi Trojan Source and C0/C1 control character stripping sanitizer.
  - Cooldown and snooze tracking preventing alert fatigue.
  - Advisory structured output schema with zero OS authority.
  - Pluggable OpenAI-compatible and Ollama runtime providers with deterministic offline fallback.
- **Linux Activity Observation (`wowie-observer`):**
  - Native Hyprland Wayland IPC adapter (`hyprctl activewindow -j`).
  - Sway / wlroots IPC tree adapter (`swaymsg -t get_tree`).
  - X11 EWMH adapter (`_NET_ACTIVE_WINDOW` via `xdotool`).
  - Strict privacy consent gates and window title redaction.
  - WebExtensions Native Messaging host binary (`wowie-browser-host`) with length-prefixed JSON protocol.
- **Custom 3D Companion Characters (`wowie-avatar`):**
  - VRM 1.0 (`VRMC_vrm`) and binary glTF 2.0 parser.
  - Security bounds validation (50 MB size cap, external URI rejection, path traversal protection).
  - Mesh geometry, normal, color, and double-sided material extraction.
  - Local character registry and companion mood reaction mapper.
- **Wayland Desktop Companion Overlay (`wowie-overlay`):**
  - Pure-Rust Wayland transparent window using `softbuffer` and `winit`.
  - 3D software rasterizer with perspective camera, Z-buffering, and directional Lambertian shading.
  - Reactive companion mood animations (Idle, Focused, Distraction Alert, Snoozed, Celebration).
  - 30 FPS redraw throttling with sub-millisecond CPU sleep in frame loops.
- **Safe Distraction Enforcement (`wowie-enforcement`):**
  - Section-demarcated `/etc/hosts` temporary blocking.
  - Developer tool immunities (`crates.io`, `github.com`, `docs.rs`, etc.).
  - Atomic tempfile replacement preventing file truncation.
  - Startup crash recovery clearing stale markers.
- **Terminal UI & Application Coordinator (`wowie-app`):**
  - Full 8-screen terminal user interface with keyboard navigation.
  - CLI subcommands (`goal`, `session`, `status`, `character`, `tick`, `demo`).
  - Standalone 3D overlay launcher.
- **Release Packaging & Distribution:**
  - Arch Linux `PKGBUILD`, `.desktop` launcher, and scalable SVG / PNG application icons.
  - Full dependency inventory (`THIRD_PARTY_NOTICES.md`) verifying 290 permissive open-source crates.
  - WebExtension companion (`extensions/browser/`) for Chrome, Brave, Chromium, and Firefox.
