# E2E Test Infra: Wowie AI Coach (`crates/wowie-coach`)

## Test Philosophy
- Opaque-box, requirement-driven per ORIGINAL_REQUEST.md (R1-R4) and docs/AI_BEHAVIOR.md.
- 100% offline self-contained execution without external network sockets or APIs.
- Methodology: Systematic 4-tier approach (Category-Partition, Boundary Value Analysis, Pairwise Combinatorial Testing, Real-World Workloads).

## Feature Inventory
| # | Feature | Source (Requirement) | Tier 1 | Tier 2 | Tier 3 | Tier 4 |
|---|---------|---------------------|:------:|:------:|:------:|:------:|
| 1 | CoachContext Contract | R1 (Structured Context) | >=5 | >=5 | ✓ | ✓ |
| 2 | CoachResponse Contract | R1 (Structured Output) | >=5 | >=5 | ✓ | ✓ |
| 3 | Closed Action Allowlist | R4 (Security Boundaries) | >=5 | >=5 | ✓ | ✓ |
| 4 | Error Taxonomy | R2, R4 (Error Handling) | >=5 | >=5 | ✓ | ✓ |
| 5 | Delimiter Isolation | R1, R4 (Prompt Injection Defense) | >=5 | >=5 | ✓ | ✓ |
| 6 | Prompt Builder | R1, R3 (Prompt Construction) | >=5 | >=5 | ✓ | ✓ |
| 7 | Tone & Personality System | R3 (Tone & Preferences) | >=5 | >=5 | ✓ | ✓ |
| 8 | Deterministic Cooldown Tracker | R3 (Cooldown & Suppression) | >=5 | >=5 | ✓ | ✓ |
| 9 | Pluggable Provider & Mock | R2 (Provider Abstraction) | >=5 | >=5 | ✓ | ✓ |
| 10 | OpenAiCompatibleClient | R2 (Local/Remote Provider) | >=5 | >=5 | ✓ | ✓ |
| 11 | Deterministic Fallback Engine | R2 (Graceful Degradation) | >=5 | >=5 | ✓ | ✓ |
| 12 | Unified CoachEngine Facade | R1-R4 (Integrated Facade) | >=5 | >=5 | ✓ | ✓ |

## Test Architecture
- Test Runner: `cargo test -p wowie-coach`
- Location: `crates/wowie-coach/tests/`
  - `e2e_tier1_features.rs`: Feature isolation & happy paths
  - `e2e_tier2_boundaries.rs`: Extremes, boundaries, limits, and invalid inputs
  - `e2e_tier3_combinations.rs`: Pairwise feature interactions & cross-system flows
  - `e2e_tier4_workloads.rs`: Real-world end-to-end focus session scenarios
- Semantics: All tests pass completely offline (exit code 0)

## Coverage Thresholds
- Tier 1: >=60 test cases (covering all 12 feature areas)
- Tier 2: >=60 test cases (boundary limits and edge conditions)
- Tier 3: >=25 test cases (major pairwise feature combinations)
- Tier 4: >=10 realistic application-level scenarios
- Total E2E Target: >=155 comprehensive tests
