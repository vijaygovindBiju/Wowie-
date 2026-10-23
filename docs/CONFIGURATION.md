# Configuration proposal

Defaults are conservative proposals. Exact numeric thresholds remain unresolved until measured and approved. Configuration must explain trade-offs and remain easy to reset.

| Setting | Proposed default | Notes |
|---|---|---|
| Goals and next action | User-created; no sample goal preselected | Wowie may suggest wording; user confirms. |
| Monitoring | Off until user starts and consents to a session | Visible indicator; pause available. |
| Observation scope | App identity only if adapter can provide it; title/browser data separately opt-in | No screenshot or page-text capture in MVP. |
| Sampling interval | Event-driven preferred; evaluate 2–5 seconds only where polling is required | Candidate, not verified. Expose resource-aware choices after testing. |
| AI mode | Rules/no AI baseline | Local provider optional; cloud disabled until explicit configuration. |
| AI data scope | Current session summary only | No persistent conversation memory by default. |
| Avatar | No avatar / notification fallback | User may import a supported model after validation milestone. |
| Character name/personality | Neutral, user-configurable | Tone changes presentation, never policy. |
| Voice | Off | Separate opt-in; local/cloud source disclosed. |
| Intervention threshold | Conservative; exact duration/confidence unresolved | Must not trigger enforcement on classification alone. |
| Coaching cooldown | Enabled; numeric value unresolved | User can tune within safe rate limits. |
| Break allowance | User chooses per commitment | Idle and planned break are not distraction. |
| Snooze | User-selected bounded interval | Does not authorize a block. |
| Enforcement | Disabled until user configures an adapter and rule | Each action previewed and confirmed; visible stop path. |
| History retention | User-selectable; default period unresolved | Provide export/delete; raw event retention minimized. |
| Resource limits | Bounded; numeric CPU/memory/model/renderer values unresolved | Must be measured on reference devices. |
| Diagnostics | Local, redacted; sharing off | No activity telemetry by default. |

Any configuration change that expands collection, cloud transmission, permission scope, or enforcement requires a clear explanation and confirmation. Safe reset returns monitoring off, AI cloud off, enforcement off, and notifications available.

