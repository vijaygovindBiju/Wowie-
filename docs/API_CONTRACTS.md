# Proposed internal API contracts

These are illustrative data contracts, not executable code. Final serialization and naming follow the accepted stack decision. All timestamps use UTC; identifiers are opaque; strings are bounded and treated as untrusted.

## Activity observation

    ActivityObservation {
      observation_id,
      session_id,
      observed_at,
      source: "platform_app" | "window_title" | "browser_origin" | "manual",
      app_id?,
      window_title?,
      origin?,
      idle_state: "active" | "idle" | "unknown",
      consent_scope_id,
      expires_at
    }

Contract: producer supplies only enabled fields; consumer rejects events for a paused/ended session, expired consent, impossible timestamp, or unknown source. Raw values are not instructions. expires_at bounds transient use.

## Assessment request and response

    AssessmentRequest {
      request_id,
      goal: { title, next_action },
      session: { elapsed_seconds, break_state },
      activity: [{ app_id?, origin?, category?, duration_seconds }],
      user_rules: [{ scope, relevance }],
      response_limit
    }

    AssessmentResponse {
      request_id,
      relevance: "relevant" | "possibly_unrelated" | "unknown",
      confidence: 0.0_to_1.0,
      rationale,
      evidence_ids: [],
      question?,
      action_candidate?: "remind" | "ask_context" | "none"
    }

Contract: provider cannot add action types. Validate bounds, IDs, confidence, and response size; preserve provider/model/version provenance. AI response is advisory and cannot authorize enforcement.

## Policy decision

    PolicyDecision {
      decision_id,
      outcome: "no_action" | "ask_context" | "remind" | "offer_user_action",
      reason_codes: [],
      evidence_ids: [],
      allowed_choices: [],
      cooldown_until?,
      policy_version
    }

Only deterministic policy produces this record. An offer_user_action is not execution authorization.

## Enforcement request and result

    EnforcementRequest {
      request_id,
      action: "temporary_site_block",
      target: { exact_origin_or_user_rule_id },
      session_id,
      duration_seconds,
      expires_at,
      user_confirmation_id,
      policy_decision_id
    }

    EnforcementResult {
      request_id,
      state: "applied" | "denied" | "unavailable" | "failed" | "removed",
      adapter_id,
      effective_scope?,
      expires_at?,
      stop_handle?,
      reason_code?
    }

Contract: adapter verifies the one-use confirmation, target, expiry, maximum duration, active session, and capability before changing state. Stop handle is opaque and revocable. Result reports actual capability, not promised coverage.

## Character presentation event

    CharacterEvent {
      event_id,
      kind: "idle" | "encourage" | "ask_context" | "progress" | "celebrate",
      intensity: "subtle" | "normal",
      expires_at,
      fallback: "notification" | "none"
    }

Contract: event excludes raw activity content and policy authority. Renderer may ignore unsupported event or animation. It cannot return a policy mutation or system action.

