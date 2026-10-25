# Decision register

Status vocabulary is defined in [docs index](README.md). Detailed rationale lives in linked ADRs.

| ID | Decision | Current recommendation | Status | Record |
|---|---|---|---|---|
| ADR-001 | Application architecture | Modular Cargo workspace across 8 focused crates with `#![forbid(unsafe_code)]`. | **accepted** | [Application boundaries](adr/ADR-001-application-architecture.md) |
| ADR-002 | Character format and renderer | VRM 1.0 / binary glTF with pure Rust software rasterizer (`softbuffer` + `winit`) and Wayland transparent overlay. | **accepted** | [Character format & rendering](adr/ADR-002-character-format-rendering.md) |
| ADR-003 | AI execution strategy | Deterministic core; prompt injection defense with delimiter isolation; offline heuristic & pluggable OpenAI/Ollama provider. | **accepted** | [AI execution](adr/ADR-003-ai-execution.md) |
| ADR-004 | Linux activity observation | Native Hyprland (`hyprctl activewindow -j`), Sway (`swaymsg -t get_tree`), and X11 platform adapters with domain extraction. | **accepted** | [Activity observation](adr/ADR-004-activity-observation.md) |
| ADR-005 | Enforcement mechanism | Scoped, reversible hosts file distraction blocking with demarcated sections, atomic tempfile writes, and crash recovery. | **accepted** | [Enforcement](adr/ADR-005-enforcement.md) |
| D-006 | Local persistence | SQLite with transaction pool, atomic migrations, and active session singleton constraint. | **accepted** | [TECH_STACK](TECH_STACK.md), [DATA_MODEL](DATA_MODEL.md) |
| D-007 | Desktop UI framework | Dual presentation: interactive 8-screen terminal UI (`wowie-app`) + floating Wayland 3D companion overlay (`wowie-overlay`). | **accepted** | [TECH_STACK](TECH_STACK.md) |
| D-008 | Project independence | Wowie is a standalone desktop application. | **accepted** | [ACCOUNTABILITY_RULES](ACCOUNTABILITY_RULES.md) |
| D-009 | Presentation milestones | Completed both Phase 1 (notifications & ASCII fallback) and Phase 5 (real 3D overlay). | **accepted** | [ROADMAP](ROADMAP.md), [PRODUCT_REQUIREMENTS](PRODUCT_REQUIREMENTS.md) |

## Resolved Architecture Questions

1. **Linux Integration:** Hyprland native integration via IPC socket/`hyprctl activewindow -j`, with fallback to Sway tree parsing and X11 `xdotool`.
2. **Language & UI:** Rust 2021 workspace. UI consists of an interactive 8-screen terminal UI and a lightweight transparent Wayland window (`wowie-overlay`).
3. **Resource Footprint:** Overlay software rasterizer runs at ~24% CPU @ 30 FPS with <35MB RSS memory footprint.
4. **Local Persistence:** SQLite via `rusqlite` with atomic migration versioning.
5. **Character Pipeline:** VRM 1.0 / binary GLB with 50MB file cap, URI rejection, glTF scene graph traversal, and Z-buffered Lambertian software rasterization.
6. **Enforcement:** Demarcated block sections in `/etc/hosts` with automatic backup and crash recovery sanitization.
