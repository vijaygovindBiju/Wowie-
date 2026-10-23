# Enforcement architecture

**Status:** proposed, with mechanism deferred. Enforcement is a narrow optional subsystem. Accountability policy is authoritative in [ACCOUNTABILITY_RULES.md](ACCOUNTABILITY_RULES.md).

## Supported action types

Initial action type is a user-requested, temporary website block through one future adapter selected by ADR-005. A reminder or snooze is a coaching action, not system enforcement. App termination, forced lockout, global firewall changes, disabling essential tools, and irreversible actions are out of scope.

## Authorization sequence

1. A commitment is active and user has configured an eligible target rule.
2. Assessment may propose a mismatch and action candidate.
3. Deterministic policy validates consent, evidence, target allowlist, maximum duration, caps, and adapter capability.
4. UI previews exact target, effect, scope, duration/expiry, limitations, and stop method.
5. User explicitly confirms. Confirmation creates a one-use authorization bound to the request and expiry.
6. Adapter revalidates and applies action, reports result, and emits audit event.
7. Action is removed on user stop, expiry, session end, or shutdown where technically possible.

The AI never invokes adapters. No hidden or background action is permitted.

## Consent, reversibility, and user override

Consent is per action or a clearly scoped user-configured rule with a visible confirmation policy; MVP default is per-action confirmation. Every action is time-limited and can be stopped from the main UI. Global emergency stop disables monitoring and attempts to clear all active actions. Wowie must not make itself impossible to stop. If a user intentionally re-enables a block after stopping, that is a new action and authorization.

## Conflicts and failure modes

If user rules conflict, explicit allow/essential-tool exclusions win over block. If target resolution is ambiguous, deny. If adapter is absent, permission is revoked, network/browser state cannot be read, or cleanup fails, report status and leave essential access available. Do not silently escalate to privileged system settings. At shutdown or crash recovery, inspect tracked active actions and provide a safe removal path.

## Platform limits and audit

Browser extensions may not cover other browsers or alternate access paths. DNS, VPN, DoH, direct IP, and browser policy can bypass a web rule. System-wide hosts/DNS/firewall actions can affect unrelated apps and require privilege, so are not an MVP default. Maintain minimal audit records of request, consent, target scope, expiry, result, and stop reason; do not log unrelated browsing.

## Testing

Test with a fake adapter first: absent consent, expired token, wrong target, excessive duration, conflicting rule, repeated request, user stop, expiry, session end, app crash, adapter denial, and cleanup failure. Platform tests must verify exact scope and that no broad network configuration changes occur. See [Testing](TESTING.md).

