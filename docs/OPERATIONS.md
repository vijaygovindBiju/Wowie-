# Planned operations

No install, run, build, packaging, or maintenance commands have been implemented or tested. This document describes future operator needs, not current instructions.

## Installation and development setup

Proposed after stack and packaging decisions: publish supported package artifacts and checksums; document required runtime libraries and optional integrations; provide a clean development setup with pinned toolchain, formatter, linter, type check, and tests. Any shell commands added here must be labeled **proposed / untested** until verified on a clean target system.

## Running and configuration

The application should start with monitoring off unless resuming a session through explicit confirmation. First-run setup explains data storage, monitoring, AI provider, and optional character separately. Show current monitoring/AI/enforcement status and capability. Settings changes should be durable and exportable.

## Logs and diagnostics

Planned local logs should contain application lifecycle, adapter status, errors, and policy reason codes without raw window titles, URLs, screenshots, credentials, or prompts by default. Diagnostics export is user-triggered and previewable. No telemetry upload is planned by default.

## Troubleshooting

Future guide should cover unsupported compositor/session, denied permissions, missing portal backend, browser connector disconnection, notification service failure, AI provider timeout, invalid avatar, GPU/renderer failure, and unresolved temporary action cleanup. Each case needs a safe disable/recovery step.

## Shutdown, upgrades, and uninstall

Graceful shutdown cancels observation/inference/render tasks, attempts to stop temporary enforcement, commits necessary records, and reports any cleanup issue. Upgrade path uses versioned database migrations and keeps a recoverable backup before irreversible migration if appropriate. Uninstall clearly distinguishes program files, user history, credentials, and avatar assets; offer explicit preservation or deletion and document package-manager behavior.

## Packaging and release

Packaging is undecided. Arch-first development is proposed, with Flatpak or distro packages evaluated later. Release material should include version, supported environment matrix, dependency/license inventory, known limitations, data handling, upgrade/uninstall behavior, and verified commands. AppStream metadata can support software-center presentation after project metadata and packaging exist.

