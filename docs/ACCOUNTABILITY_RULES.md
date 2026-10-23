# Accountability rules

This is the sole authoritative home for Wowie's product-wide behavior and safety rules. The architecture, flows, and TODOs should reference these rules rather than redefine them. All are proposed until approved.

## System-wide rules

1. Wowie is a standalone application, not a ForgeX plugin.
2. Users choose their own ambitions, goals, and commitments.
3. Users can upload a supported character model or use Wowie without one.
4. Character presentation is independent of the accountability engine.
5. Monitoring is visible, permission-based, and easy to pause.
6. Entertainment is not automatically a violation.
7. An AI suggestion is not an authorized system action.
8. All enforcement actions go through a deterministic policy layer.
9. AI cannot override user-configured safety constraints.
10. Consequences are bounded, transparent, and recoverable.
11. The application remains usable when AI services are unavailable.
12. Local activity data is not sent to a cloud model by default; cloud processing requires explicit configuration.
13. Users can correct classifications and review relevant history.
14. Monitoring, AI inference, and character rendering have configurable resource budgets.
15. The application has a reliable stop, pause, and recovery path.
16. Never claim an unimplemented feature is complete.
17. Never mark a TODO complete without meeting its acceptance criteria.
18. Document assumptions and unresolved decisions instead of silently inventing answers.

## Decision and intervention policy

An intervention is eligible only when there is an active user-created commitment, the user enabled the relevant monitoring scope, an available signal supports a sustained possible mismatch, no break/idle/correction/snooze exemption applies, and policy rate limits permit it. Exact confidence and duration values are open pending calibration. Activity category alone is insufficient. If evidence or platform state is ambiguous, remain silent or ask for context; never enforce.

AI classifications are advisory inputs. Policy is deterministic and can be tested without AI. The user can correct an assessment and choose whether a correction applies to the current item, session, or future rule. A correction suppresses repeated prompts for a configurable cooldown. The system should explain what evidence informed a prompt.

## Breaks, snooze, and user rules

The user defines allowed break duration/count when creating a commitment. Idle time is not itself a violation. A planned break suspends distraction timing. Snooze has a bounded user-selected duration and suppresses prompts; it does not create an enforcement action. User allow/deny/context rules are explicit, editable, and visible. No app/site is hard-coded as universally distracting.

## Consequences and limits

Default consequence is a contextual reminder. Do not impose consequences simply because entertainment was detected. Any block requires a relevant commitment, an enabled user rule, sufficient evidence, a supported adapter, preview, and explicit confirmation. Penalties are disabled by default. Never use physical harm, deprivation of necessities, public humiliation, or financial penalties without informed explicit authorization; for MVP, exclude those categories entirely. A missed commitment gets a recovery conversation, not punishment.

Rate limits must prevent repeated alerts or blocks; numeric caps are deferred but must be approved before implementation. The user can end the session, disable coaching, stop enforcement, pause monitoring, or quit. These controls cannot be removed or hidden by AI, policy, or character presentation.

## Recovery and audit

On missed commitment, ask whether schedule/goal/context changed; offer a smaller next action or new commitment. Record only the user-selected note and necessary outcome. Audit events distinguish observation, interpretation, policy decision, user confirmation, action result, and correction. History should support explanation and deletion without retaining excessive raw activity.

## Dependency rule

The deterministic decision engine and its consent/policy invariants are central to the architecture and must not depend on avatar format, renderer, or AI provider. The no-AI and no-avatar paths are required.

