# Contributing

Wowie is in its documentation and planning phase. There is no application code or established coding standard yet. Contributions to planning are welcome; implementation should begin only after the project owner explicitly approves the documentation and implementation plan.

## Principles

- Keep Wowie independent of ForgeX and avoid coupling product concepts to another project's implementation.
- Prefer small modules and explicit contracts inside one application over premature services.
- Keep AI advisory. Deterministic policy owns permissions and system actions.
- Use privacy-minimizing, visible, user-configurable behavior with a reliable pause/stop path.
- Build and test on Linux desktop environments that are explicitly named in the support matrix; never infer universal Linux support.
- Keep character presentation independent from goals, sessions, classification, and policy.
- Use established formatter, linter, type-checking, and test tools once language and framework decisions are accepted. Record versions and commands in `docs/OPERATIONS.md` after they are verified.

## Expected development workflow

1. Read `docs/README.md`, `docs/PRODUCT_REQUIREMENTS.md`, and the relevant architecture, component, policy, and security documents.
2. Link work to stable requirement IDs and TODO IDs. Clarify or update the requirement before implementing behavior that is not specified.
3. For a change affecting boundaries, persistence, privacy, platform integration, AI authority, character formats, or enforcement, propose an ADR under `docs/adr/` and update `docs/DECISIONS.md` before or with the change.
4. Work in a focused change with a small acceptance scope. Keep UI, observation, AI, policy, and enforcement responsibilities separate.
5. Add or update tests at the appropriate unit, integration, platform, privacy, and user-flow levels. Do not claim a TODO complete until its listed acceptance criteria and verification method pass.
6. Update docs, configuration examples, migrations, and compatibility notes with behavior changes.
7. Submit a pull request with purpose, linked IDs, user-visible behavior, privacy/security impact, test commands and results, platform coverage, and known limitations. Request review for any permission expansion or new enforcement capability.

## Coding standards (to be established)

The language, formatter, lint rules, type system, error conventions, and supported compiler versions are undecided. The selected stack ADR must establish these before code is merged. Until then, do not add manifests or scaffolding under the documentation plan.

## Testing expectations

Every behavior change should have a deterministic test where practical. Policy must be tested without AI or an operating-system integration. Platform-specific behavior needs a named desktop/session test matrix and graceful behavior when an integration is absent. See [Testing](docs/TESTING.md).

## Documentation maintenance

Keep one authoritative home for each rule. Update requirement traceability, TODO acceptance criteria, decision status, and relative links when changing a contract. Use “proposed,” “accepted,” “deferred,” or “implemented” accurately; accepted means approved as a project decision, not implemented.

