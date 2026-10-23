# ADR-002: Character format and rendering boundary

- **Status:** proposed; format and runtime selection deferred pending prototype
- **Date:** 2026-10-24

## Context

Users must bring their own supported 3D model. A format should provide useful humanoid/avatar conventions while keeping renderer replaceable, asset safety measurable, and the no-avatar path functional.

## Alternatives considered

- VRM 1.0, glTF 2.0/GLB plus app conventions, VRM 0.x, FBX, Blender source files, OBJ/static images.
- Godot, Qt Quick 3D, custom graphics runtime, or a web renderer.

## Recommendation

Prototype VRM 1.0 as the first candidate because its glTF basis and avatar metadata/expressions provide more interoperable conventions than raw glTF. Do not accept this format or a renderer until an asset corpus validates import, animation mappings, resource bounds, Linux overlay behavior, and licensing. Keep generic format capability out of the accountability core. Static image and notifications remain fallbacks.

## Rationale and consequences

VRM is focused on humanoid avatars and can express metadata, license information, humanoid mapping, expressions, and spring-bone behavior. It does not guarantee that every file is safe, lightweight, compatible, or licensed for every use. glTF is broadly runtime-neutral but does not dictate playback behavior, so Wowie would need its own conventions. Supporting multiple versions/formats multiplies test and security surface.

## Evidence and assumptions

[VRM feature overview](https://vrm.dev/en/vrm/vrm_features/), [VRM format](https://vrm.dev/en/vrm/gltf/format/), [VRM Public License](https://vrm.dev/en/licenses/1.0/), [Khronos glTF registry](https://registry.khronos.org/glTF/), and [Godot import documentation](https://docs.godotengine.org/en/stable/tutorials/assets_pipeline/importing_3d_scenes/index.html). Runtime-specific VRM 1.0 compatibility and license interpretation for bundled dependencies remain unverified.

