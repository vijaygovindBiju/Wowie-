# ADR-001: Modular single-application architecture

- **Status:** accepted
- **Date:** 2026-10-24

## Context

Wowie needs a desktop UI, session domain, platform monitoring, policy, optional AI, local persistence, enforcement, and optional character presentation. Early distributed services would add deployment and failure complexity without a multi-user/server requirement.

## Alternatives considered

- Modular single process with adapters.
- Multiple local processes for UI, monitoring, and renderer.
- Local client/server or cloud-first service architecture.

## Recommendation

Start as one desktop application with logical modules and explicit interfaces. Use a separate process only where the renderer's crash/resource isolation or a platform integration proves a concrete need.

## Rationale and consequences

This keeps setup and offline use simple and supports deterministic testing. A single process creates shared crash/resource risk, so untrusted asset parsing must be bounded and potentially isolated. Modules must not reach around contracts. No network service is required for the MVP.

## Evidence and assumptions

The product is single-user/local-first and has no approved sync requirement. This is a design inference, not a measured implementation result. Revisit if process isolation is needed for asset parsing or renderer stability.

