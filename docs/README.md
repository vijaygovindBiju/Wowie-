# Wowie Documentation

Wowie is an implemented, modular Cargo workspace across 8 crates providing a goal-aware AI desktop companion on Linux with activity awareness, contextual coaching, custom 3D character avatars, and user-controlled distraction enforcement.

## Reading Order

1. [Project Introduction](../README.md) — Vision, implemented capabilities, architecture, and quickstart commands.
2. [User Manual](USER_MANUAL.md) — Comprehensive guide to focus workflows, TUI, 3D overlay, browser extension, and CLI.
3. [Product Requirements](PRODUCT_REQUIREMENTS.md) — Authoritative user and system requirements (`FR-*`, `NFR-*`).
4. [Architecture](ARCHITECTURE.md), [Components](COMPONENTS.md), [API Contracts](API_CONTRACTS.md), and [Data Model](DATA_MODEL.md) — Technical boundaries and module contracts.
5. [User Flows](USER_FLOWS.md) — Implemented end-to-end focus session and coaching interaction paths.
6. [Accountability Rules](ACCOUNTABILITY_RULES.md) — Authoritative coaching thresholds, consent, recovery, and safety rules.
7. [Activity Monitoring](ACTIVITY_MONITORING.md), [AI Behavior](AI_BEHAVIOR.md), and [Enforcement](ENFORCEMENT.md) — Subsystem rules, domain extraction boundaries, and hosts-file safety.
8. [Avatar System](AVATAR_SYSTEM.md), [Privacy/Security](PRIVACY_SECURITY.md), and [Configuration](CONFIGURATION.md) — Character validation, 3D overlay rendering, and threat models.
9. [Technology Stack](TECH_STACK.md) and [Decision Register](DECISIONS.md), then the [ADRs](adr/) — Evaluated options and accepted architectural decision records.
10. [Testing](TESTING.md), [Operations](OPERATIONS.md), [Roadmap](ROADMAP.md), and [TODO](TODO.md) — Verification records, test results, and milestone tracking.
11. [Contributing](../CONTRIBUTING.md) and [Security Policy](../SECURITY.md) — Contributor expectations and vulnerability reporting.

## Authoritative Locations

| Topic | Authority |
|---|---|
| Product behavior and requirement IDs | [PRODUCT_REQUIREMENTS.md](PRODUCT_REQUIREMENTS.md) |
| User journeys | [USER_FLOWS.md](USER_FLOWS.md) |
| Accountability and safety rules | [ACCOUNTABILITY_RULES.md](ACCOUNTABILITY_RULES.md) |
| Component boundaries and system flows | [ARCHITECTURE.md](ARCHITECTURE.md), [COMPONENTS.md](COMPONENTS.md), and [API_CONTRACTS.md](API_CONTRACTS.md) |
| Persisted entities and retention | [DATA_MODEL.md](DATA_MODEL.md) |
| Activity signals and their limits | [ACTIVITY_MONITORING.md](ACTIVITY_MONITORING.md) |
| AI authority, uncertainty, and memory | [AI_BEHAVIOR.md](AI_BEHAVIOR.md) |
| System actions and blocking | [ENFORCEMENT.md](ENFORCEMENT.md) |
| Character import and 3D overlay | [AVATAR_SYSTEM.md](AVATAR_SYSTEM.md) |
| Data threat model & sandbox rules | [PRIVACY_SECURITY.md](PRIVACY_SECURITY.md) |
| Settings defaults | [CONFIGURATION.md](CONFIGURATION.md) |
| Technology evaluations | [TECH_STACK.md](TECH_STACK.md) |
| Decision status | [DECISIONS.md](DECISIONS.md) and individual [ADRs](adr/) |
| Delivery phases and tasks | [ROADMAP.md](ROADMAP.md), [TODO.md](TODO.md) |
| Verification strategy & test reports | [TESTING.md](TESTING.md) |
| Operator procedures | [OPERATIONS.md](OPERATIONS.md) |

## Status Vocabulary

- **Implemented**: Present in code and verified by automated test suites or live desktop verification.
- **Accepted**: Approved architectural decision record governing active implementations.
- **Deferred**: Explicitly scheduled for future opt-in milestones (e.g. GPU hardware acceleration, WebExtension companion).
- **Proposed**: Early design exploration undergoing review.
