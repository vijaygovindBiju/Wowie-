# Activity monitoring

**Status:** proposed behavior; no monitor exists. Monitoring policy is governed by [Accountability rules](ACCOUNTABILITY_RULES.md) and [Privacy/security](PRIVACY_SECURITY.md).

## Signal levels and limitations

1. **Application identity** may identify the foreground app when the chosen platform adapter exposes it.
2. **Window title** can add context, but titles may be blank, misleading, personalized, contain private text, or be attacker-controlled.
3. **Browser origin/URL** requires a separate browser integration or permission. An origin says which site is open, not what the user is doing there.
4. **Screen-content understanding** would require a separately approved future capability such as capture, OCR, or vision inference. It is not in the MVP and must never be implied by a title-only or URL-only signal.

No signal can reliably distinguish all tutorials, research, entertainment, and unrelated material. Wowie should ask for context or remain silent when evidence is ambiguous.

## Consent and sampling

Monitoring is off outside an explicitly started session and must have a visible state indicator. Onboarding explains exact metadata and purpose. App/window metadata and browser integration have separate toggles. Capture/screenshot permission is not requested in MVP. A pause/revoke action stops reads and clears transient buffers. Proposed initial sampling is event-driven where possible; otherwise evaluate a 2–5 second interval in a later feasibility spike. This interval is an initial hypothesis, not a selected setting or benchmark.

Do not retain a high-frequency raw stream by default. Normalize events into coarse time intervals, discard stale/duplicate data, and persist only the minimal summaries needed for coaching and user review. Buffer duration and retention are open design decisions.

## Session boundaries and classification

An observation is attached to a session only when its monitoring consent is active. Record source and timestamp. Distinguish active application, app switch, idle, known break, unknown, and source unavailable. A session pause suspends observation and timers. Idle time does not become distraction. App switching alone is not evidence of distraction.

Classification confidence must include source quality and context. Unknown/low-confidence content cannot trigger a block. User corrections supersede an interpretation at the scope the user selects and are not silently generalized. Treat educational video, documentation, and research as relevant where user context or explicit rule supports that conclusion.

## Linux and permissions

X11 and Wayland have different security models. Under Wayland, ambient cross-client details are compositor-mediated and vary among environments. XDG portals provide consent-mediated interfaces for supported capabilities such as screen casting; they do not establish a universal low-cost active-window metadata API. A compositor-specific integration may be required. The MVP must name tested desktop environments and report unsupported/denied capability honestly. No broad claim of “Linux support” is allowed without a compatibility matrix.

Browser URL access should use a separately reviewed extension/connector with the narrowest permissions and a clear install/removal path. Do not inject page scripts or collect page contents for the metadata-only MVP.

## Resource budget and failure behavior

The settings model will eventually bound sampling, buffer size, CPU, memory, network calls, and inference frequency. Numeric ceilings await measurement on reference devices. On adapter failure, permission revocation, suspend/resume ambiguity, or stale events, mark source unavailable and stop mismatch escalation. Continue session timing and user-entered tracking where possible. Never infer an activity during a monitoring gap.

