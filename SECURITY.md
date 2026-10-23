# Security policy

## Current status

Wowie is in planning. There is no deployed application, operational security program, or dedicated security contact. Do not send sensitive reports to an invented address. Until a public reporting channel is established, use the repository's private vulnerability-reporting facility if available; otherwise contact the project owner through the same channel used to grant repository access. Avoid public disclosure of exploitable details before maintainers can respond.

## Planned security expectations

- Treat window titles, URLs, page text, model files, model metadata, AI outputs, and imported archives as untrusted input.
- Request only permissions needed for a named feature; explain collection and processing before enabling monitoring or cloud AI.
- Keep credentials in the operating system's credential store where available. Never put tokens in source, logs, exported diagnostics, or model prompts.
- Validate imported character assets with bounded parsers and resource limits; do not execute embedded scripts or load arbitrary plugins from an avatar.
- AI output cannot invoke a shell or OS API. Structured requests pass through deterministic policy and explicit user authorization.
- Make monitoring visible. Provide pause, stop, deletion, and recovery paths that remain available during integration failures.
- Pin or constrain dependencies, review updates, record licenses, and produce a dependency inventory before release.

See [Privacy and security threat model](docs/PRIVACY_SECURITY.md), [Avatar system](docs/AVATAR_SYSTEM.md), [AI behavior](docs/AI_BEHAVIOR.md), and [Enforcement](docs/ENFORCEMENT.md).

