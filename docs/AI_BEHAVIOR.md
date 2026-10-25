# AI coach behavior

**Status:** Implemented in `crates/wowie-coach`. AI is strictly optional, advisory, and isolated behind deterministic policy and prompt quarantine rules. Approved actions are constrained by [Enforcement](ENFORCEMENT.md).

## Inputs and outputs

When configured, AI receives only the current goal/next action, session context, minimized activity labels/evidence, user correction state, and relevant tone preferences. It returns a structured assessment: relevance label, confidence, short rationale, uncertainty, optional question, and optional action candidate from a closed enum. It must not receive unrestricted histories or secrets by default.

The AI may help break down a goal, explain an assessment, phrase a reminder, or ask for context. It may not diagnose, shame, infer sensitive traits, rewrite commitments, decide penalties, change settings, or control the operating system.

## Intervene or remain silent

Use a conservative rule: do not prompt if there is no active commitment, monitoring is paused, activity is idle/known break, evidence is weak, the user corrected/snoozed it, or a cooldown applies. For ambiguous content, ask a brief context question only if configured; otherwise remain silent. Do not label entertainment as a violation solely by category.

Exact confidence and sustained-duration thresholds are deferred until calibration. Until approved, initial prototypes must use transparent deterministic fixtures and must not claim reliable content classification.

## Learning versus unrelated content

Application names and URL categories are only clues. A video site may host a tutorial, a coding site may host unrelated content, and a browser title may be misleading. Prefer explicit user labels and goal context, preserve an unknown class, and offer correction without arguing. A user can establish temporary or persistent rules and inspect their scope.

## Memory, tone, and cooldowns

Default conversation memory is session-scoped and minimal. Persistent preference memory is explicit, inspectable, editable, and deletable. Personality settings affect wording, not authority. The user chooses name, tone, directness, humor, and voice; neutral text remains available. Repeated prompts have deterministic cooldowns and daily/session caps. Exact values await product-owner review.

## Local/cloud execution

No-AI mode is a supported operating mode. A local provider is optional and resource-bounded. Cloud inference is off until explicit configuration; before each provider is enabled, identify fields sent, purpose, provider, and retention caveat. Provider timeouts, missing credentials, network failures, malformed schemas, and model refusal degrade to rules and notifications.

## Untrusted content and prompt injection

Window titles, URLs, webpage content, imported model metadata, and user-provided descriptions are data, not instructions. Delimit them in provider requests and explicitly tell models to treat them as untrusted. Never place credentials in prompts. Validate output against strict schemas and discard unknown fields. A malicious title must not change policy or expand allowed actions.

## Action boundary

AI can suggest a typed action candidate from an allowlist. The deterministic policy validates the user rule, session, target, duration, and consent. A UI confirmation is required for any enforcement request. AI has no shell, process execution, filesystem, browser-control, window-control, or network action tool.

