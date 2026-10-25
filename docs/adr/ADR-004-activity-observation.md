# ADR-004: Permissioned Linux activity observation

- **Status:** accepted
- **Date:** 2026-10-25

## Context

Wowie requires observing active application and window metadata during consented focus sessions to assist users in maintaining their commitments. Linux environments (Wayland compositors vs X11) have fundamentally different security architectures with no universal unprivileged global active-window API. Relying on window titles for web awareness has strict limitations.

## Alternatives Considered

1. **Global X11-Only Observation (`xdotool` / `XQueryTree`):**
   - Incompatible with modern Wayland desktop environments.
   - *Decision:* Maintained as legacy fallback only (`X11Adapter`).
2. **Screen-Content Capture / Continuous Video Streaming (XDG Desktop Portal Screencast):**
   - High privacy invasion, high CPU/GPU resource overhead, unnecessary for metadata classification.
   - *Decision:* Rejected for MVP.
3. **Compositor-Specific IPC Adapters:**
   - Zero-privilege, native performance, discrete metadata queries.
   - *Decision:* **Accepted. Implemented `HyprlandAdapter` using `hyprctl activewindow -j`.**
4. **Browser Title Regex Extraction vs WebExtension Native Messaging:**
   - Window titles fail in 6 common scenarios (generic titles, multi-tab spoofing, fullscreen video, Electron PWAs, private browsing, subdomain confusion).
   - *Decision:* **Accepted Native Messaging bridge as recommended browser integration path.**

## Decision

1. **Architecture:** Use a pluggable `PlatformObserver` trait in `wowie-observer` with runtime compositor auto-detection (`HyprlandAdapter`, `X11Adapter`, `MockObserver`).
2. **Hyprland Native Implementation:** Query the compositor's UNIX domain socket or CLI (`hyprctl activewindow -j`) returning `title`, `class`, and `initialTitle`.
3. **Privacy Invariants:**
   - Explicit user consent required before observation begins.
   - Regex redaction of sensitive tokens (emails, tokens, file paths) in titles.
   - Zero background monitoring outside active focus sessions.
4. **Browser Integration Recommendation:**
   - WebExtension Native Messaging bridge (`wowie-browser-bridge`) communicating origin-only JSON frames over stdin/stdout.
   - Strict avoidance of broad DOM or network permissions (`<all_urls>`, `webRequest`).

## Consequences

- Full, reliable native support on Arch Linux / Hyprland out of the box.
- Transparent reporting when running on unsupported Wayland compositors (GNOME, KDE) with graceful fallback to manual session tracking.
