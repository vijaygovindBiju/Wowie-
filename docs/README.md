# Wowie documentation

All behavior described here is planned unless a section explicitly says otherwise. The repository is currently documentation-only.

## Reading order

1. [Project introduction](../README.md) — vision, status, and boundaries.
2. [Product requirements](PRODUCT_REQUIREMENTS.md) — authoritative user and system requirements (`FR-*`, `NFR-*`).
3. [User flows](USER_FLOWS.md) — end-to-end interaction paths.
4. [Accountability rules](ACCOUNTABILITY_RULES.md) — authoritative coaching thresholds, consent, recovery, and safety rules.
5. [Architecture](ARCHITECTURE.md), [Components](COMPONENTS.md), [API contracts](API_CONTRACTS.md), and [Data model](DATA_MODEL.md) — proposed technical boundaries and structures.
6. [Activity monitoring](ACTIVITY_MONITORING.md), [AI behavior](AI_BEHAVIOR.md), and [Enforcement](ENFORCEMENT.md) — subsystem rules.
7. [Avatar system](AVATAR_SYSTEM.md), [Privacy/security](PRIVACY_SECURITY.md), and [Configuration](CONFIGURATION.md) — character, data, threat, and settings constraints.
8. [Technology stack](TECH_STACK.md) and [Decision register](DECISIONS.md), then the [ADRs](adr/) — evaluated options and decision status.
9. [Testing](TESTING.md), [Operations](OPERATIONS.md), [Roadmap](ROADMAP.md), and [TODO](TODO.md) — delivery and verification plan.
10. [Contributing](../CONTRIBUTING.md) and [Security policy](../SECURITY.md) — contributor and reporting expectations.

## Authoritative locations

| Topic | Authority |
|---|---|
| Product behavior and requirement IDs | [PRODUCT_REQUIREMENTS.md](PRODUCT_REQUIREMENTS.md) |
| User journeys | [USER_FLOWS.md](USER_FLOWS.md) |
| Accountability and safety rules | [ACCOUNTABILITY_RULES.md](ACCOUNTABILITY_RULES.md) |
| Component boundaries and system flows | [ARCHITECTURE.md](ARCHITECTURE.md), with contracts in [COMPONENTS.md](COMPONENTS.md) and [API_CONTRACTS.md](API_CONTRACTS.md) |
| Persisted entities and retention | [DATA_MODEL.md](DATA_MODEL.md) |
| Activity signals and their limits | [ACTIVITY_MONITORING.md](ACTIVITY_MONITORING.md) |
| AI authority, uncertainty, and memory | [AI_BEHAVIOR.md](AI_BEHAVIOR.md) |
| System actions and blocking | [ENFORCEMENT.md](ENFORCEMENT.md) |
| Character import and renderer | [AVATAR_SYSTEM.md](AVATAR_SYSTEM.md) |
| Data threat model | [PRIVACY_SECURITY.md](PRIVACY_SECURITY.md) |
| Settings defaults | [CONFIGURATION.md](CONFIGURATION.md) |
| Technology evaluations | [TECH_STACK.md](TECH_STACK.md) |
| Decision status | [DECISIONS.md](DECISIONS.md) and individual [ADRs](adr/) |
| Delivery phases and tasks | [ROADMAP.md](ROADMAP.md), [TODO.md](TODO.md) |
| Verification strategy | [TESTING.md](TESTING.md) |
| Proposed operator procedures | [OPERATIONS.md](OPERATIONS.md) |

## Status vocabulary

**Proposed** means a recommendation awaits project-owner review. **Accepted** means approved as a design decision but not necessarily built. **Deferred** means deliberately postponed. **Implemented** means present in code and verified to its stated criteria. No application feature is currently implemented.

