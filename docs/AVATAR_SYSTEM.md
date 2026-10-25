# Bring-your-own character system

**Status:** Milestone 1 implemented. Custom 3D character import, validation, local registry, software 3D rendering pipeline, and Wayland desktop companion overlay are operational.

## Format candidates and selection

- **VRM 1.0 (Selected for 3D Companion):** Preferred candidate for humanoid avatar profile because it is glTF-based and adds humanoid, expression, metadata, material, and secondary-motion conventions. Implemented and verified via `wowie-avatar` and `wowie-overlay`.
- **glTF 2.0 / GLB:** Broad runtime-neutral asset format with meshes and keyframe data. Supported by the underlying geometry validator and loader.
- **VRM 0.x:** Planned compatibility extension for later iterations.
- **Static image / Terminal ASCII art:** Operational low-resource fallback.

## Implemented 3D Architecture & Rendering Pipeline

The character system is split into two isolated layers:

```
[User GLB/VRM Asset]
        │
        ▼
[crates/wowie-avatar]
  ├─ ModelValidator: Security bounds check (50MB, no external URIs/scripts)
  ├─ Binary Geometry Extraction: Positions ([f32; 3]), Normals, Vertex Colors, Triangles
  ├─ Model3D Normalization: Extent scaling & bounding-box recentering
  └─ CharacterRegistry: Safe app-managed local asset storage & metadata tracking
        │
        ▼ (Model3D + Mood events via OverlayBridge)
[crates/wowie-overlay]
  ├─ Windowing: winit 0.30 Wayland transparent window + Hyprland auto-floating rules
  ├─ Blitting: softbuffer 0.4 presentation
  ├─ Renderer3D:
  │   ├─ Math Pipeline: Perspective projection camera, LookAt view matrix, Yaw/Pitch orbit
  │   ├─ Rasterizer: Z-buffer depth testing, triangle bounding box scan, edge functions
  │   ├─ Lighting: Directional Gouraud shading + Ambient + Specular highlights
  │   └─ Mood Animation Transforms:
  │       ├─ Idle: Gentle breathing bobbing and subtle sway
  │       ├─ Focused: Upright, attentive posture with minimal motion
  │       ├─ Distraction Alert: Urgent pulse and tilt
  │       ├─ Snooze: Relaxed tilted posture
  │       └─ Celebration: Victory hop, spin, and celebratory color
  └─ HUD Pill: Glass badge indicating companion name, mood badge, and focus status
```

## Import and validation workflow

Choose file → check extension, MIME/signature and file size → parse in a constrained importer with allocation and time bounds → inspect scene graph, mesh counts, vertices, bones, texture dimensions/total decoded size, animation channels, external references, and required extensions → validate supported profile → extract metadata/license declarations → present preview/report → user confirms storage and terms → copy into app-managed asset store and hash file.
- **Security Invariant:** Do not execute scripts, shaders, plugins, or arbitrary external references. Reject archives/path traversal, malformed binary lengths, unsupported required extensions, unreasonable complexity, and resource exhaustion.
- **Strict Caps:** 50MB file size, 500,000 maximum vertices, 1,000,000 maximum triangles.

## Companion Mood Vocabulary & State Mapping

The accountability engine maps session state to a discrete event vocabulary:
1. `CompanionMood::Neutral` (Idle/Resting)
2. `CompanionMood::Focused` (Active deep focus session)
3. `CompanionMood::Concerned` (Distraction alert / mismatch)
4. `CompanionMood::Relaxed` (Snoozed / scheduled break)
5. `CompanionMood::Celebrating` (Session completed / goal milestone achieved)
6. `CompanionMood::Thinking` (Analyzing activity context)

These states drive both the 3D model transforms in `wowie-overlay` and the terminal ASCII art in `wowie-app`.

## User Controls & Hotkeys

In the desktop overlay window:
- **`Q`** or **`Esc`**: Quit overlay.
- **`H`**: Toggle overlay visibility (show/hide).
- **`M`**: Cycle companion mood state.
- **`C`**: Cycle active character model from the local registry.
- **Mouse Left Drag**: Orbit camera around character.
- **Mouse Wheel**: Zoom camera in and out.

## Metadata, licensing, and storage

Show creator, title, declared license/usage and redistribution/avatar-use settings when present. Metadata is not legal proof; user confirms they have rights to use the file. Wowie does not redistribute user assets. Models are stored locally under `~/.local/share/wowie/characters/`, with export and permanent deletion commands provided (`wowie character list`, `wowie character import <path>`).

## Fallback & Graceful Degradation

If the Wayland compositor or display server is unavailable, or if model loading fails, Wowie degrades gracefully:
- Terminal UI displays real-time ASCII companion art.
- System notifications (`notify-send` / `DesktopNotifier`) deliver focus coaching.
- Core accountability, SQLite session tracking, and goal evaluation operate unimpeded.
