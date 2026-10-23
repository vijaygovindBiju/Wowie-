# ADR-003: Optional, advisory AI execution

- **Status:** proposed
- **Date:** 2026-10-24

## Context

Goal interpretation and natural coaching may benefit from AI, but focus sessions must function when models, credentials, network, or inference capacity are unavailable. Activity data is sensitive and AI output is untrusted.

## Alternatives considered

- No AI; deterministic rules and user-authored mappings only.
- Required cloud AI.
- Local AI only.
- Deterministic core plus optional local and separately opted-in cloud providers.

## Recommendation

Use the deterministic policy and a basic rules path as the product baseline. Define a provider-neutral structured assessment interface. Evaluate local inference first. Cloud providers remain disabled unless the user explicitly configures them and reviews the exact fields sent.

## Consequences

Offline operation remains possible, but rules-only explanations may be less nuanced. Local models use user hardware and have variable quality/resource use; cloud offers may be more capable but introduce privacy, cost, credential, and availability concerns. AI cannot authorize or execute operating-system actions.

## Evidence and assumptions

[Ollama API overview](https://github.com/ollama/ollama/blob/main/docs/api/introduction.mdx) documents local and cloud endpoint modes. It does not establish model suitability or resource budgets for Wowie; measure those later. User preference for local/cloud balance is not yet known.

