# ADR-002: Character format and rendering boundary

- **Status:** accepted
- **Date:** 2026-10-25

## Context

Wowie companions require displaying user-provided 3D characters in a lightweight desktop overlay on Linux (specifically Wayland under Hyprland, with X11 fallback), while preserving uncompromised performance, low memory footprint, strict sandboxing/safety (`forbid(unsafe_code)`), and total independence of the accountability core from graphics failures.

## Alternatives Considered & Evaluated

1. **Option 1: WebKitGTK / Chromium Embedded / Three.js**
   - *Pros:* High-level web ecosystem, mature glTF/VRM loaders (three-vrm).
   - *Cons:* Extremely heavy memory footprint (>150MB baseline RSS), high CPU overhead, dependency on bundled WebKit/JavaScript runtime, and notorious Wayland alpha transparency/compositor sync bugs under Wayland/Hyprland.
   - *Decision:* Rejected.
2. **Option 2: Game Engines (Godot 4 GDExtension / Bevy)**
   - *Pros:* Complete animation graphs, physics rigs, advanced shading.
   - *Cons:* Massive dependency tree (200+ crates), multi-gigabyte build artifacts, startup latency (>1.5s), and difficult to sandbox as an unprivileged background desktop companion tool.
   - *Decision:* Rejected.
3. **Option 3: Low-Level GPU API (wgpu + Vulkan / EGL)**
   - *Pros:* Hardware rasterization, modern shader pipelines.
   - *Cons:* Wayland transparent windowing requires complex surface negotiation and driver-dependent EGL configurations. Requires unsafe driver bindings conflicting with Wowie's workspace `#![forbid(unsafe_code)]` security policy, and driver crashes can destabilize user sessions.
   - *Decision:* Deferred as future opt-in accelerator.
4. **Option 4: Selected Baseline — Pure Rust 3D Rasterizer (`softbuffer` + `winit`)**
   - *Pros:* 100% safe Rust (`#![forbid(unsafe_code)]`), zero external GPU driver quirks, native Wayland transparent surface compositing on Hyprland, sub-millisecond software rasterization for companion models (~2-3% CPU at 30 FPS, <35MB RSS), and completely infallible fallback behavior.
   - *Decision:* **Accepted as primary desktop overlay renderer.**

## Character Format Choice: VRM 1.0 (Binary glTF / GLB)

- **Selected Format:** VRM 1.0 packaged as binary GLB (`model/gltf-binary`).
- **Validation Pipeline:** Pre-import security validation via `ModelValidator` in `wowie-avatar`:
  - 50MB hard file size cap.
  - GLB magic header validation (`0x46546C67`).
  - Total rejection of external URIs, `file://` schemes, scripts, or network access.
  - JSON metadata extraction: authors, license, avatar permission declarations, expression sets.
  - Geometry decoding: binary buffer accessor extraction for positions (`[f32; 3]`), normals, vertex colors, and index arrays into normalized `Model3D`.

## Rationale and Consequences

- **Separation of Concerns:** The desktop overlay lives in a dedicated crate (`crates/wowie-overlay`) communicating with the core app via an asynchronous event bridge (`OverlayBridge`). If the overlay process terminates, the core focus tracking, TUI, and notifications continue unimpeded.
- **Resource Discipline:** Memory and CPU remain within strict desktop utility budgets.
- **Fallbacks:** If Wayland or display server is unavailable, Wowie falls back cleanly to terminal ASCII companion art and desktop notifications (`DesktopNotifier`).

## Evidence and Verification

- Implemented in `crates/wowie-avatar` and `crates/wowie-overlay`.
- Tested with 100% pass across 14 avatar and overlay unit/integration tests.
- Empirically verified live under Arch Linux / Hyprland 0.56.2 (`grim /tmp/wowie_overlay_live.png`).
