# Decision register

Status vocabulary is defined in [docs index](README.md). “Proposed” is not owner approval. Detailed rationale lives in linked ADRs.

| ID | Decision | Current recommendation | Status | Record |
|---|---|---|---|---|
| ADR-001 | Application architecture | Modular single-process desktop application; no MVP microservices. | proposed | [Application boundaries](adr/ADR-001-application-architecture.md) |
| ADR-002 | Character format and renderer | Evaluate VRM 1.0 first with a candidate renderer; keep format/renderer separate from policy; prototype required. | proposed | [Character format](adr/ADR-002-character-format-rendering.md) |
| ADR-003 | AI execution strategy | Deterministic core; optional provider-neutral local AI; cloud only explicit opt-in. | proposed | [AI execution](adr/ADR-003-ai-execution.md) |
| ADR-004 | Linux activity observation | One named platform adapter, visible consent, no universal Wayland claim; browser connector separate. | proposed | [Activity observation](adr/ADR-004-activity-observation.md) |
| ADR-005 | Enforcement mechanism | Typed deterministic authorization, temporary/reversible action; select a single adapter only after feasibility/security review. | deferred | [Enforcement](adr/ADR-005-enforcement.md) |
| D-006 | Local persistence | SQLite recommended; retention and encryption specifics open. | proposed | [TECH_STACK](TECH_STACK.md), [DATA_MODEL](DATA_MODEL.md) |
| D-007 | Desktop UI framework | Compare GTK, Qt, Tauri and target overlay behavior in feasibility work; no final selection. | deferred | [TECH_STACK](TECH_STACK.md) |
| D-008 | Project independence | Wowie is standalone and not a ForgeX plugin. | proposed | [ACCOUNTABILITY_RULES](ACCOUNTABILITY_RULES.md) |
| D-009 | Initial presentation milestone | Notification-first vertical slice; custom 3D model is a follow-on milestone, not removed from product goal. | proposed | [ROADMAP](ROADMAP.md), [PRODUCT_REQUIREMENTS](PRODUCT_REQUIREMENTS.md) |

## Unresolved decisions

1. Which Linux desktop/session is the first supported integration and which metadata API is available without intrusive permissions?
2. Which programming language and UI framework fit team capability, packaging, overlays, and resource goals?
3. What are acceptable CPU, memory, disk, sampling, and AI latency budgets on reference hardware?
4. What is the default history retention period and what local encryption approach is practical?
5. Is a browser extension acceptable for optional URL observation/blocking, and which browser(s) matter first?
6. Which renderer can safely import the chosen VRM profile and meet overlay/resource requirements? Which license obligations apply to libraries and sample assets?
7. What exact confidence and sustained-duration thresholds minimize false prompts without making the product ineffective?
8. Should the first release include any cloud AI provider, given no mandatory cloud/subscription requirement?
9. What project security-reporting channel and release ownership will be established?

