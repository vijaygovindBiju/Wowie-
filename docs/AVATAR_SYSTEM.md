# Bring-your-own character system

**Status:** planned long-term capability. The first MVP vertical slice may use notifications; custom character support remains a product goal and gets a dedicated follow-on milestone.

## Format candidates

- **VRM 1.0:** preferred candidate for first humanoid avatar profile because it is glTF-based and adds humanoid, expression, metadata, material, and secondary-motion conventions. Compatibility with any chosen Linux renderer is unproven.
- **glTF 2.0/GLB:** broad runtime-neutral asset format with meshes, skins, and keyframe data, but it does not define avatar semantics or animation playback policy.
- **VRM 0.x:** possible compatibility extension later; separate profile and parser behavior required.
- **FBX, OBJ, Blender project files:** not initial direct-import targets. Their conversion/runtime complexity and differing animation/material support broaden risk.
- **Static image:** low-resource fallback, not a model format.

## Import and validation workflow

Choose file → check extension, MIME/signature and file size → parse in a constrained importer with allocation and time bounds → inspect scene graph, mesh counts, vertices, bones, texture dimensions/total decoded size, animation channels, external references, and required extensions → validate supported profile → extract metadata/license declarations → present preview/report → user confirms storage and terms → copy into app-managed asset store and hash file. Do not execute scripts, shaders, plugins, or arbitrary external references. Reject archives/path traversal, malformed binary lengths, unsupported required extensions, unreasonable complexity, and resource exhaustion. Parsing may need a separate restricted process after threat review.

Numeric file-size and complexity ceilings require reference-device measurement; they must be configurable only within safe hard limits. Initial model size should be conservatively capped, and decoded texture/GPU cost should be separately bounded.

## Feature profile and animation

Require a supported skeleton/rig for animated humanoid behavior. Map available idle and expression capabilities to a small app-owned event vocabulary: neutral/idle, acknowledge, encourage, concern, celebration, and attention. Missing expressions/gestures use neutral pose, simple fade, or notification fallback. Do not assume imported animation names have standard semantics. Validate animation duration/channel counts and avoid uncontrolled physics simulation. Voice is separate, opt-in, and not required for animation.

## Metadata, licensing, and storage

Show creator, title, declared license/usage and redistribution/avatar-use settings when present. Metadata is not legal proof; user confirms they have rights to use the file. Wowie does not redistribute user assets. Preserve relevant metadata through export when technically possible. Asset library licenses and bundled sample assets require separate review. Store models locally under an app-controlled directory, not in the database; provide export and permanent removal. Never upload a model automatically.

## Isolation, privacy, and extension

Treat assets as untrusted. Limit parser and renderer resource use; disable network access from imported content; prevent path traversal and unrestricted external URI loads. Keep registry, validation profile, format loader, renderer, and policy contracts separate. A new format/renderer requires a compatibility and security review. The accountability engine receives only presentation events, never model code or renderer permissions.

## Fallback

User may choose no 3D character, static image, or desktop notification. Import failure cannot block goals or sessions. A renderer crash cannot pause policy or monitoring invisibly; the UI reports fallback status and exposes controls.

