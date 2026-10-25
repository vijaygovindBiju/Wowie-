# Enforcement architecture

**Status:** Implementation complete for `LinuxEnforcementAdapter` and `EnforcementPolicyEngine` in `crates/wowie-enforcement`. Mechanisms and audit findings documented below.

## Supported action types

Initial action type is a user-requested, temporary website block through `LinuxEnforcementAdapter` (or browser integration). A reminder or snooze is a coaching action, not system enforcement. App termination, forced lockout, global firewall changes, disabling essential tools, and irreversible actions are strictly out of scope.

## Hardcoded Essential Tool Immunities

To prevent accidental developer lockout or system malfunction, the enforcement engine enforces hardcoded, non-overridable immunities for core developer and system tools:
- Package managers & compilers: `cargo`, `rustc`, `rustup`, `gcc`, `clang`, `pacman`, `yay`
- Version control & dev tooling: `git`, `code`, `alacritty`, `kitty`, `tmux`, `nvim`, `vim`
- Core documentation & repositories: `crates.io`, `docs.rs`, `github.com`, `gitlab.com`, `archlinux.org`
- System essentials: `localhost`, `127.0.0.1`, `::1`

## Authorization sequence

1. A focus commitment is active and user has configured an eligible target rule.
2. Context assessment detects a sustained mismatch and proposes an action candidate.
3. Deterministic policy engine validates consent, evidence, immunities, maximum duration (<= 120 minutes), and single-use confirmation token.
4. UI previews exact target, effect, scope, expiry, and stop method.
5. User explicitly confirms within 120 seconds. Confirmation creates a one-use token bound to request and expiry.
6. Adapter validates and applies action, reports result, and emits audit event.
7. Action is removed on user stop, expiry, session end, app shutdown, or crash recovery.

The AI never invokes adapters. No hidden or background action is permitted.

## Technical Security & Integrity Audit: `/etc/hosts`

A deep audit was conducted on using hosts-file editing on Linux for distraction blocking:

1. **Privilege Boundary:**
   - `/etc/hosts` is owned by `root:root` with mode `0644`.
   - Modifying `/etc/hosts` from an unprivileged desktop process fails with `PermissionDenied`.
   - Wowie's `LinuxEnforcementAdapter` defaults to an unprivileged path (`$TMPDIR/wowie_enforced_blocks.txt` or user-configured resolver blocklist), and surfaces an explicit, actionable error if pointed at an unwriteable system file.
2. **Original File Preservation (Demarcated Sections):**
   - The adapter MUST NEVER truncate or overwrite the hosts file.
   - All managed block entries are enclosed within strict demarcation markers:
     ```text
     # --- BEGIN WOWIE TEMPORARY DISTRACTION BLOCKLIST ---
     # Auto-generated and automatically cleared on session end
     127.0.0.1 youtube.com
     ::1 youtube.com
     # --- END WOWIE TEMPORARY DISTRACTION BLOCKLIST ---
     ```
   - All lines outside these markers (existing localhost mappings, custom host entries, router aliases) are strictly preserved unmodified.
3. **Crash Recovery & Startup Sanitization:**
   - If the application crashes, is killed (`SIGKILL`), or power fails during an active focus block, the blocklist entries remain on disk.
   - On startup, `LinuxEnforcementAdapter` inspects the target file.
   - If the file is a shared file (contains entries outside Wowie's markers), startup recovery strips **only** the demarcated section without deleting the file.
   - This prevents stale blocks from persisting across reboots while ensuring user host mappings are never destroyed.
4. **Automatic Backup & Restoration:**
   - Before modifying any shared system file, the adapter creates a backup at `<path>.wowie_backup`.
   - The adapter exposes `restore_from_backup()`, allowing instant recovery if file writing is interrupted or corrupted.
5. **Repeated Starts & Idempotency:**
   - Multiple session starts or restarts cleanly replace or remove the demarcated block without duplicating entries or bloating the file.
6. **Graceful Drop Cleanup:**
   - When `LinuxEnforcementAdapter` is dropped, its `Drop` implementation safely removes only the demarcated section.

## Platform limits and audit

- **Bypass Paths:** DNS-over-HTTPS (DoH), DNS-over-TLS (DoT), VPN tunnels, or direct IP access can bypass hosts-file blocking in modern browsers.
- Product wording must never promise absolute network isolation; it is a friction tool to help users stay committed.
- Minimal audit records are persisted locally: request timestamp, target domain, expiry, and stop reason. Unrelated browsing is never inspected or logged.

## Automated Verification

Automated regression tests in `crates/wowie-enforcement/tests/enforcement_tests.rs`:
- `test_hosts_preservation_and_demarcated_section`: Preserves existing host entries.
- `test_crash_recovery_strips_wowie_section_without_deleting_file`: Verifies non-destructive startup recovery.
- `test_repeated_starts_idempotency_and_backup_restoration`: Verifies backup creation and restoration.
- `test_essential_tool_immunity_enforced`: Rejects blocks on developer essentials.
- `test_token_single_use_and_expiration`: Rejects reused or expired authorization tokens.
