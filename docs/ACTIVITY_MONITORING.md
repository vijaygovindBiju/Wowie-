# Activity monitoring

**Status:** Operational for Linux desktop observation via `crates/wowie-observer` (Hyprland native `hyprctl activewindow -j`, X11 `xdotool`, and synthetic mock). Monitoring policy is strictly governed by [Accountability rules](ACCOUNTABILITY_RULES.md) and [Privacy/security](PRIVACY_SECURITY.md).

## Signal levels and limitations

1. **Application identity (`app_id` / `window_class`):** Identifies the foreground app when exposed by the desktop compositor. Highly reliable for differentiating development tools (e.g., Alacritty, VS Code) from general-purpose browsers (e.g., Firefox, Chromium).
2. **Window title:** Adds descriptive context, but titles may be blank, truncated, personalized, contain sensitive information, or be attacker-controlled.
3. **Browser origin/URL:** Requires dedicated browser integration. Window title heuristics can extract basic domains, but have documented failure boundaries.
4. **Screen-content understanding:** Out of scope for MVP; title or app metadata must never be conflated with visual understanding.

No signal alone can reliably distinguish whether a user is reading a Rust tutorial on YouTube or watching entertainment. Wowie preserves uncertainty, solicits user context, and avoids premature distraction escalation.

## Browser Domain Extraction Limits & Failure Scenarios

Analysis and testing revealed 6 key limitations when relying solely on desktop window titles for web domain awareness:

1. **Generic Page Titles:**
   - Single-page apps, dashboards, intranet portals, and login screens often display titles like `"Dashboard"`, `"Sign In"`, or `"Home"` without mentioning the domain or service name.
   - Result: Domain extractor cannot identify the target origin.
2. **Multi-Tab Title Spoofing & Background Media:**
   - The desktop compositor only exposes the title of the *currently active tab* in the focused window.
   - Background audio/video streaming in another tab (or another window) cannot be detected via active-window titles, or conversely, switching tabs briefly can mislead the classifier.
3. **Fullscreen Video Titles Omitting Platform:**
   - Fullscreen video playback in web browsers (e.g., HTML5 video players) often switches the window title to the media title only, stripping the suffix `" - YouTube"` or `" - Netflix"`.
4. **Desktop PWAs and Electron Applications:**
   - Progressive Web Apps (PWAs) and Electron applications (e.g., Slack, Spotify, Discord, Notion) run with custom window titles that mimic native apps without standard browser title conventions.
5. **Private / Incognito Browsing:**
   - Many browsers suppress or simplify window titles in private/incognito mode to protect user privacy, omitting page titles or URLs entirely.
6. **Subdomain and Path Ambiguity:**
   - Window titles cannot differentiate between educational subdomains and entertainment sections of the same top-level domain (e.g., `docs.rs` vs standard sites, or `github.com/rust-lang` vs unrelated web content).

## Proposed Appropriately Permissioned Browser Integration: Native Messaging Bridge

To resolve title extraction limitations safely without broad surveillance permissions, the recommended design is a lightweight WebExtension communicating via WebExtensions Native Messaging:

- **Component Name:** `crates/wowie-browser-bridge` (or companion extension).
- **Communication Channel:** Standard input / standard output (`stdin`/`stdout`) using length-prefixed JSON frames mediated by the browser's native messaging host protocol.
- **Narrowest Necessary Permission Scope:**
  - Manifest permission: `"tabs"` (specifically `activeTab`) or `"declarativeNetRequest"` for reversible blocking.
  - **Forbidden Permissions:**
    - NO `"webRequest"` or network interception.
    - NO `"all_urls"` or `<all_urls>` script injection.
    - NO access to page cookies, DOM storage, passwords, form contents, or navigation history.
- **Privacy Boundary & Origin Minimization:**
  - The extension transmits ONLY:
    ```json
    {
      "origin": "https://docs.rs",
      "is_active_window": true,
      "tab_id": 42
    }
    ```
  - Full URLs (paths, query parameters, auth tokens, search queries) are stripped in the extension sandbox before sending.
  - User consent toggle in Wowie settings allows enabling or disabling browser reporting at any time.

## Consent and sampling

- Monitoring is disabled outside an explicitly started focus session and displays an active indicator in the UI.
- Sampling rate: Event-driven compositor signals where supported (Hyprland socket), or throttled 2-second debounce in polling loops.
- No high-frequency raw stream is persisted. Raw events are aggregated into coarse intervals, sanitized with regex redaction, and bounded in memory.
- Instant pause/emergency stop revokes monitoring immediately and flushes transient buffers.

## Linux Compositor Compatibility Matrix

| Compositor / Desktop | Protocol / Interface | Integration Status | Signal Quality |
|---|---|---|---|
| **Hyprland** | UNIX IPC socket (`hyprctl activewindow -j`) | Native (`HyprlandAdapter`) | High (instant active window metadata) |
| **Sway / wlroots** | `swaymsg -t get_tree` | Planned follow-on | High |
| **GNOME (Wayland)** | Mutter D-Bus / XDG Portal | Under evaluation | Restricted by Wayland security model |
| **KDE Plasma (Wayland)** | KWin Scripting / D-Bus | Under evaluation | Medium |
| **X11 (Any WM)** | EWMH / `_NET_ACTIVE_WINDOW` | Native (`X11Adapter`) | High |
