# Roadmap

Phases are proposed gates. Passing a gate means its evidence exists; it does not mean the phase has begun. Implementation awaits project-owner approval.

| Phase | Scope | Entry criteria | Exit criteria |
|---|---|---|---|
| 0. Foundation and decisions | Approve product baseline, stack, Linux signal strategy, policy semantics, threat model, and support target. | Documentation review begins. | Owner accepts or revises key ADRs; unresolved questions have owners; numeric resource and retention targets are set or explicitly deferred. |
| 1. Thin vertical slice | Local goals, commitment/session lifecycle, manual/mock activity feed, deterministic policy, notification, history. | Phase 0 exit; language/UI/storage choices accepted. | End-to-end session works with fixtures; policy works without AI; pause, correction, break, review, and restart recovery pass. |
| 2. Linux observation | One named desktop/session integration with visible consent, active app/window metadata, graceful unavailable mode. | Phase 1 exit; platform API and permission model proven on target environment. | Signal quality and limits documented; false-positive/missed-event fixtures pass; indicator and pause verified on target matrix. |
| 3. Goal relevance and AI options | Rule-based baseline, optional local provider experiment, cloud provider behind explicit opt-in, structured outputs. | Data contracts and privacy review accepted. | Offline path complete; timeout/injection/malformed-output tests pass; cloud data disclosure is exact; AI cannot invoke OS actions. |
| 4. Temporary enforcement | One reversible, user-requested, explicitly scoped website-block adapter, possibly browser extension if chosen. | Platform feasibility and abuse review; separate enforcement ADR accepted. | Preview/consent/expiry/stop/override/audit flow passes; absence/failure leaves access available; support is narrowly stated. |
| 5. Custom character milestone | Import and render one selected format with bounded validation and fallback; personality/gesture mapping. | Format and renderer ADR accepted after compatibility spike; licensing review. | User asset imports safely within measured limits; missing animation degrades gracefully; rendering can be disabled without affecting policy. |
| 6. Release hardening | Packaging, upgrades, uninstall, accessibility, compatibility matrix, security review, resource profiling. | Core MVP criteria pass. | Reproducible release candidate, dependency/license inventory, privacy/security review, tested support statement, rollback/uninstall procedure. |

## Small vertical slice

Phase 1 should prove the core product loop using a deterministic fake observation source and desktop notification, without waiting for robust Wayland activity discovery, cloud AI, blocking, or 3D rendering. This exposes product and policy issues cheaply. Phase 2 replaces the fake source with exactly one named real desktop integration. A static/simple character can test presentation wiring, while full bring-your-own 3D import remains an explicit Phase 5 milestone rather than being dropped from the vision.

## Optional enhancements

Additional compositor adapters, browser URL extension, voice, richer animated expressions, additional formats/renderers, cross-platform support, sync, and advanced content understanding are optional and must not delay the offline, controllable core. Each needs its own requirement and decision review.

