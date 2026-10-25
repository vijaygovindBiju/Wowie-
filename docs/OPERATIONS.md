# Wowie Operations & Release Guide

**Status:** Fully Implemented and Verified for Arch Linux / Linux Desktops.

---

## 1. Clean Development Environment Walkthrough (`W-007`)

A new contributor can build, test, and run Wowie without requiring root privileges, system modifications, or cloud dependencies.

### Prerequisites

On Arch Linux:
```bash
sudo pacman -S --needed base-devel git sqlite
# If Rust is not installed:
rustup default stable
rustup component add rustfmt clippy
```

### Building from Source

```bash
# Clone the repository
git clone https://github.com/vijaygovindBiju/Wowie-.git
cd Wowie-

# Build all 8 crates in release mode
cargo build --release --locked --workspace
```

### Running Quality Assurance Gates

All checks must pass with zero errors and zero warnings:

```bash
# 1. Formatting check
cargo fmt --check

# 2. Workspace compilation and target check
cargo check --workspace --all-targets

# 3. Strict Clippy linter with pedantic and nursery rules
cargo clippy --workspace --all-targets -- -D warnings

# 4. Full test suite (unit, integration, adversarial security, and persistence tests)
cargo test --workspace
```

### Running Wowie Commands

```bash
# Run the full interactive terminal user interface (8 screens)
cargo run -p wowie-app -- run

# Run the automated end-to-end focus session demo
cargo run -p wowie-app -- demo

# Launch the lightweight transparent 3D companion overlay
cargo run -p wowie-overlay

# Inspect active companion and session state via CLI
cargo run -p wowie-app -- status

# Start a focus session via CLI
cargo run -p wowie-app -- session start
```

---

## 2. Arch Linux Packaging & Distribution (`W-035`)

Wowie provides a standard, reproducible Arch Linux package adhering to Arch Linux Packaging Guidelines.

### Package Artifacts

Located in `packaging/arch/`:
- `PKGBUILD`: Package recipe defining dependencies, build commands, tests, and file installation paths.
- `wowie.desktop`: Standard XDG desktop entry with category `Utility;Development;Office;` and action entries.
- `wowie.svg`: High-resolution scalable vector icon.
- `icons/wowie.png`: 256x256 rendered raster icon.

### Building the Arch Package

To build the package locally using `makepkg`:

```bash
cd packaging/arch
makepkg -efd
```

This compiles the release binaries, runs the full test suite in fakeroot, strips unneeded symbols, and outputs `wowie-0.1.0-1-x86_64.pkg.tar.zst`.

### Installed File Placement (FHS / XDG Compliance)

| Destination Path | Permissions | Purpose |
|---|---|---|
| `/usr/bin/wowie` | 0755 (rwxr-xr-x) | Primary binary (TUI, CLI, session coordinator, demo) |
| `/usr/bin/wowie-overlay` | 0755 (rwxr-xr-x) | Standalone Wayland 3D transparent companion window |
| `/usr/bin/wowie-browser-host` | 0755 (rwxr-xr-x) | WebExtensions Native Messaging host bridge |
| `/usr/share/applications/wowie.desktop` | 0644 (rw-r--r--) | Desktop application launcher |
| `/usr/share/icons/hicolor/scalable/apps/wowie.svg` | 0644 (rw-r--r--) | Scalable vector application icon |
| `/usr/share/icons/hicolor/256x256/apps/wowie.png` | 0644 (rw-r--r--) | Raster application icon |
| `/usr/lib/mozilla/native-messaging-hosts/org.wowie.browser_host.json` | 0644 (rw-r--r--) | Firefox native messaging manifest |
| `/etc/chromium/native-messaging-hosts/org.wowie.browser_host.json` | 0644 (rw-r--r--) | Chromium native messaging manifest |
| `/etc/opt/chrome/native-messaging-hosts/org.wowie.browser_host.json` | 0644 (rw-r--r--) | Google Chrome native messaging manifest |
| `/usr/share/licenses/wowie/LICENSE-MIT` | 0644 (rw-r--r--) | MIT License text |
| `/usr/share/licenses/wowie/LICENSE-APACHE` | 0644 (rw-r--r--) | Apache 2.0 License text |

### Clean Separation of User Data vs. Package Files

Wowie strictly enforces the XDG Base Directory specification. No user data is ever written to `/usr/` or system directories:

1. **User Databases & Assets (`$XDG_DATA_HOME/wowie` / `~/.local/share/wowie`):**
   - `wowie.db`: SQLite database storing user ambitions, goals, commitments, sessions, and reviews.
   - `characters/`: User-imported custom VRM 1.0 and glTF 3D character models and textures.
   - `characters.json`: Registry metadata for imported character models.
2. **User Configuration (`$XDG_CONFIG_HOME/wowie` / `~/.config/wowie`):**
   - `settings.json`: User coaching tone preferences, notification preferences, and sampling configuration.
3. **Transient Runtime IPC State (`$XDG_RUNTIME_DIR/wowie` / `/run/user/$UID/wowie`):**
   - `active_browser_origin.json`: Atomic browser origin cache read by desktop activity observers.

### Upgrade, Rollback, and Uninstall Lifecycles

- **Upgrade:**
  `sudo pacman -U wowie-0.1.0-2-x86_64.pkg.tar.zst` replaces application binaries and desktop manifests. User data in `~/.local/share/wowie` and `~/.config/wowie` is untouched. On startup, SQLite schema migrations execute idempotently.
- **Rollback:**
  `sudo pacman -U /var/cache/pacman/pkg/wowie-0.1.0-1-x86_64.pkg.tar.zst` downgrades binaries.
- **Uninstallation:**
  `sudo pacman -R wowie` removes system binaries, desktop icons, and native manifests.
  **User data is preserved by default.** If the user wishes to completely delete their history, they can explicitly run:
  ```bash
  rm -rf ~/.local/share/wowie ~/.config/wowie /run/user/$UID/wowie
  ```

---

## 3. Browser Extension & Native Messaging Setup

For accurate browser domain detection without invasive permissions, Wowie includes a dedicated WebExtension companion.

### Registering the Native Messaging Host

Run the user-level setup script without sudo:
```bash
./scripts/install-browser-host.sh
```
This registers `org.wowie.browser_host.json` in:
- `~/.config/google-chrome/NativeMessagingHosts/`
- `~/.config/chromium/NativeMessagingHosts/`
- `~/.config/BraveSoftware/Brave-Browser/NativeMessagingHosts/`
- `~/.mozilla/native-messaging-hosts/`

To remove the registration:
```bash
./scripts/install-browser-host.sh --uninstall
```

### Loading the Browser Extension

1. Open your browser:
   - **Chromium / Chrome / Brave:** Navigate to `chrome://extensions/`, enable **Developer mode**, click **Load unpacked**, and select `extensions/browser/`.
   - **Firefox:** Navigate to `about:debugging#/runtime/this-firefox`, click **Load Temporary Add-on**, and select `extensions/browser/manifest.json`.
2. Click the Wowie extension icon in your browser toolbar to verify connection status (🟢 Connected).
3. The popup UI displays the current active domain, connection status, reporting toggle, and privacy guarantees.

---

## 4. Platform Support Matrix

| Desktop Environment / Compositor | Protocol / Mechanism | Implementation Status | Live Hardware Verification |
|---|---|---|---|
| **Arch Linux + Hyprland** | UNIX IPC socket (`hyprctl activewindow -j`) | `HyprlandAdapter` | **Physically Verified Live** |
| **Sway / wlroots** | UNIX IPC tree (`swaymsg -t get_tree`) | `SwayAdapter` | Implemented; 100% automated test pass |
| **X11 (i3, bspwm, Xfce, GNOME X11)** | EWMH (`_NET_ACTIVE_WINDOW` via `xdotool`) | `X11Adapter` | Implemented; 100% automated test pass |
| **KDE Plasma (Wayland)** | KWin Scripting / D-Bus | Planned follow-on | Under evaluation |
| **GNOME (Wayland)** | Mutter D-Bus / XDG Desktop Portal | Planned follow-on | Restricted by Wayland security model |

---

## 5. Security & Privacy Controls

### Activity Privacy
- Monitoring is active **strictly during an active focus session**.
- Outside sessions, observers remain stopped.
- Window titles can be redacted to `[Redacted Title]` via user privacy settings.
- The browser extension transmits **only** `https://domain.tld`. All URL paths (`/watch?v=...`), queries, and fragments are stripped inside the browser sandbox before IPC transmission.
- Private/incognito windows are completely ignored.
- Keystrokes, screen pixels, passwords, and page DOM contents are **never** captured.

### Enforcement Safety & Crash Recovery
- Distraction blocking operates via temporary entries in `/etc/hosts` enclosed by demarcation markers:
  ```
  # --- BEGIN WOWIE TEMPORARY DISTRACTION BLOCKLIST ---
  127.0.0.1 youtube.com
  # --- END WOWIE TEMPORARY DISTRACTION BLOCKLIST ---
  ```
- Pre-existing user hosts entries are preserved byte-for-byte.
- Essential developer services (`crates.io`, `github.com`, `docs.rs`, `archlinux.org`) are immune from blocking.
- Atomic file writes via tempfile rename prevent half-written corruption.
- On startup, `wowie` audits `/etc/hosts` and removes any stale blocks left by abnormal process crashes.
- Manual emergency recovery:
  ```bash
  sudo sed -i '/# --- BEGIN WOWIE/,/# --- END WOWIE/d' /etc/hosts
  ```

---

## 6. Measured Performance Baselines

Tested on reference Arch Linux hardware (Intel Core i5-1135G7 @ 2.40GHz, Hyprland Wayland):

| Metric | Measured Baseline | Target Budget (NFR) | Status |
|---|---|---|---|
| **Cold Startup Time (`wowie --help`)** | **3.46 ms** | < 100 ms | Well within budget |
| **Native Host Startup (`wowie-browser-host`)** | **1.18 ms** | < 50 ms | Well within budget |
| **Peak Resident Memory (RSS) - Full Demo** | **8.05 MB** | < 60 MB | Well within budget |
| **Observer Sampling CPU Utilization** | **< 0.1%** | < 1.0% | Negligible load |
| **30 FPS 3D Overlay Rendering CPU** | **~24% (1 core)** | < 35% | Responsive & smooth |
| **AI Evaluation Latency (Mock Provider)** | **< 0.1 ms** | < 500 ms | Instantaneous |
| **Workspace Test Suite Execution** | **1.8 s (678 tests)** | < 10 s | 100% test pass |
