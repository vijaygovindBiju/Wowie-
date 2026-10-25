# Roadmap

**Status:** Phases 0 through 6 are **Implemented & Verified**. Wowie MVP is Release Ready.

| Phase | Scope | Entry criteria | Exit criteria | Status |
|---|---|---|---|---|
| **0. Foundation and decisions** | Product baseline, stack, Linux signal strategy, policy semantics, threat model, and support target. | Documentation review begins. | Owner accepts or revises key ADRs; unresolved questions have owners; numeric resource and retention targets are set or explicitly deferred. | **COMPLETED** |
| **1. Thin vertical slice** | Local goals, commitment/session lifecycle, manual/mock activity feed, deterministic policy, notification, history. | Phase 0 exit; language/UI/storage choices accepted. | End-to-end session works with fixtures; policy works without AI; pause, correction, break, review, and restart recovery pass. | **COMPLETED** |
| **2. Linux observation** | Native desktop/session integration with visible consent, active app/window metadata, graceful unavailable mode. | Phase 1 exit; platform API and permission model proven on target environment. | Signal quality and limits documented; false-positive/missed-event fixtures pass; indicator and pause verified on target matrix (Hyprland, Sway, X11). | **COMPLETED** |
| **3. Goal relevance and AI options** | Rule-based baseline, prompt injection defense, offline heuristic & OpenAI/Ollama compatible provider, structured outputs. | Data contracts and privacy review accepted. | Offline path complete; timeout/injection/malformed-output tests pass; cloud data disclosure is exact; AI cannot invoke OS actions (548 tests pass). | **COMPLETED** |
| **4. Temporary enforcement** | Reversible, user-requested, explicitly scoped website-block adapter with crash recovery. | Platform feasibility and abuse review; separate enforcement ADR accepted. | Preview/consent/expiry/stop/override/audit flow passes; absence/failure leaves access available; pre-existing hosts lines preserved. | **COMPLETED** |
| **5. Custom character milestone** | Import and render supported VRM 1.0 / glTF formats with bounded validation; Wayland transparent 3D overlay with mood reactions. | Format and renderer ADR accepted after compatibility spike; licensing review. | User asset imports safely within measured limits; software rasterizer runs at ~24% CPU @ 30 FPS (<35MB RSS); desktop notifications & ASCII fallback verified. | **COMPLETED** |
| **6. Release hardening** | Packaging (PKGBUILD, desktop entry), WebExtension native host companion, user documentation. | Core MVP criteria pass. | Reproducible release candidate, dependency/license inventory, privacy/security review, tested support statement, rollback/uninstall procedure. | **COMPLETED** |

## Implemented Milestones Summary

- **Phase 1 & 2:** Proved the full focus loop and integrated live Hyprland (`hyprctl activewindow -j`), Sway (`swaymsg -t get_tree`), and X11 activity tracking.
- **Phase 3:** Built `wowie-coach` with delimiter quarantine (`<untrusted_activity_evidence>`), BiDi Trojan Source stripping, deterministic timestamp cooldown tracking, and graceful fallback.
- **Phase 4:** Built `wowie-enforcement` with demarcated section editing, essential developer tool immunities, atomic tempfile renaming, and startup crash recovery.
- **Phase 5:** Built `wowie-avatar` (VRM 1.0 scene graph parser & security validator) and `wowie-overlay` (pure-Rust Wayland transparent software rasterizer with perspective camera, Z-buffering, and mood animation states).
