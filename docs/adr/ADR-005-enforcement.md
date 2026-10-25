# ADR-005: Explicit and reversible enforcement

- **Status:** accepted
- **Date:** 2026-10-25

## Context

Users may request temporary website blocking during a focus session. Linux has no assumed universal unprivileged mechanism that safely blocks a target across all browsers and network paths without privilege or browser integration. A mechanism must guarantee reversibility, essential tool immunities, non-destructive file preservation, and crash recovery.

## Alternatives Considered

1. **Global Firewall (iptables / nftables):**
   - High risk: Requires full root/CAP_NET_ADMIN privileges, can disrupt background services, difficult to roll back safely after ungraceful crashes.
   - *Decision:* Rejected.
2. **Local DNS Proxy / systemd-resolved DNS Stub:**
   - Adds architectural complexity and external dependencies.
   - *Decision:* Deferred.
3. **Hosts-File Adapter with Demarcated Section and Crash Recovery:**
   - Feasible, direct, and transparent.
   - Requires addressing privilege boundaries, existing file content preservation, startup recovery, and backup paths.
   - *Decision:* **Accepted for Linux desktop enforcement (`LinuxEnforcementAdapter`).**
4. **Browser Extension Native Messaging:**
   - Evaluated as complementary, narrow-permission solution for in-browser tab blocking and activity awareness.
   - *Decision:* **Accepted as proposed browser integration path.**

## Decision & Security Requirements

1. **Privilege Boundary:**
   - Direct writes to `/etc/hosts` fail as unprivileged desktop users (`EACCES`).
   - `LinuxEnforcementAdapter` defaults to a user-writable path (`wowie_enforced_blocks.txt`) or a user-configured DNSmasq/hosts inclusion snippet. If configured with a shared system file, it requires explicit operator permission.
2. **Demarcated Section Architecture:**
   - Must use `# --- BEGIN WOWIE TEMPORARY DISTRACTION BLOCKLIST ---` and `# --- END WOWIE TEMPORARY DISTRACTION BLOCKLIST ---`.
   - Never overwrite or truncate unrelated hosts file entries.
3. **Crash Recovery & Startup Invariant:**
   - On startup, the adapter inspects the file. If markers are found, it strips only the demarcated block and keeps the rest of the file intact.
   - Under no circumstances does crash recovery delete the entire hosts file.
4. **Automatic Backups & Manual Restore:**
   - Pre-mutation backup created at `<path>.wowie_backup`.
   - `restore_from_backup()` API provided for automated or operator-driven rollback.
5. **Hardcoded Immunities:**
   - Developer tools (`cargo`, `rustc`, `git`, `code`, `alacritty`, `docs.rs`, `crates.io`) cannot be blocked under any circumstances.
6. **Authorization:**
   - Single-use 120s confirmation token; maximum duration bounded to 120 minutes.

## Consequences

- Focus blocking is strictly bounded, predictable, and cannot compromise the user's network or developer tools.
- Modern browsers with DNS-over-HTTPS (DoH) may bypass local hosts entries; Wowie makes no false promises of uncircumventable blocking.
