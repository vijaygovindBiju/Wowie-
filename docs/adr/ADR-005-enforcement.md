# ADR-005: Explicit and reversible enforcement

- **Status:** deferred
- **Date:** 2026-10-24

## Context

Users may request temporary website blocking during a focus session. Linux has no assumed universal unprivileged mechanism that safely blocks a target across all browsers and network paths.

## Alternatives considered

- Browser extension per supported browser.
- User-managed hosts file or system DNS/firewall changes.
- Local proxy or DNS service.
- No blocking in first release; reminders only.

## Recommendation

Defer mechanism selection until a feasibility and abuse review. Prefer reminders first. If blocking is accepted, start with a narrowly scoped browser integration if its permission and installation costs are acceptable. Never silently modify global network configuration. All actions require explicit confirmation, fixed expiry, visible status, stop/override, and recovery.

## Consequences

The MVP vertical slice can ship without blocking. Any adapter may be bypassed by alternative browsers, DNS-over-HTTPS/VPN, or direct IP; product wording must not promise complete blocking. Cleanup failures need a manual recovery explanation.

## Evidence and assumptions

Browser integration options and permissions require a chosen browser and official extension API review. No mechanism has been tested. User demand for blocking versus friction-free reminders is unknown.

