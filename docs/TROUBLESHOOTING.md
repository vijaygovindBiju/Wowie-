# Troubleshooting & Operator Diagnostics

This guide covers common edge cases, desktop environment configurations, and crash recovery procedures for Wowie.

---

## 1. Desktop & Display Issues

### Issue: "Wayland display server is not available"
- **Cause:** Wowie overlay attempted to launch without an active Wayland compositor or X11 server (`WAYLAND_DISPLAY` or `DISPLAY` environment variables are unset).
- **Resolution:**
  - On Wayland: Verify `echo $WAYLAND_DISPLAY` returns `wayland-1` (or similar).
  - Headless/SSH environments: The desktop 3D overlay requires an active desktop graphical session. Run `wowie run` (TUI mode) or CLI subcommands (`wowie status`, `wowie demo`), which function without graphical servers.

### Issue: Transparent overlay shows black background instead of desktop
- **Cause:** Your compositor does not support alpha channel visual surfaces or compositor blur/transparency is disabled in compositor configuration.
- **Resolution:**
  - On Hyprland: Ensure `decoration { blur { enabled = true } }` or standard transparent window rules are enabled in `hyprland.conf`.

---

## 2. Activity Monitoring & Compositor Permissions

### Issue: Observer reports "Platform unsupported" or "Window title unknown"
- **Cause:** The desktop compositor IPC tool is not installed or accessible in `$PATH`.
- **Diagnostics:**
  - For Hyprland: Run `hyprctl activewindow -j`. It should return a valid JSON object.
  - For Sway: Run `swaymsg -t get_tree`.
  - For X11: Ensure `xdotool` is installed (`sudo pacman -S xdotool`).
- **Resolution:**
  - If using an unsupported compositor (e.g. GNOME Wayland), Wowie automatically switches to title heuristics or the browser extension companion.

---

## 3. Browser Extension & Native Messaging

### Issue: Extension shows "Offline" in popup badge
- **Cause:** The native messaging manifest is not registered for your browser, or the binary path in the manifest is invalid.
- **Diagnostics:**
  - Check whether manifest exists:
    * Chromium/Chrome/Brave: `ls -la ~/.config/google-chrome/NativeMessagingHosts/org.wowie.browser_host.json` (or `Brave-Browser`).
    * Firefox: `ls -la ~/.mozilla/native-messaging-hosts/org.wowie.browser_host.json`.
  - Check binary path inside manifest:
    ```bash
    cat ~/.config/BraveSoftware/Brave-Browser/NativeMessagingHosts/org.wowie.browser_host.json
    ```
    Ensure the path points to an executable binary.
- **Resolution:**
  - Run the installer script:
    ```bash
    ./scripts/install-browser-host.sh
    ```
  - Verify executable permissions:
    ```bash
    wowie-browser-host --ping
    ```

---

## 4. Distraction Enforcement & `/etc/hosts` Recovery

### Issue: A website remains blocked after application shutdown
- **Cause:** Wowie process was forcibly terminated (`SIGKILL` or power outage) while an enforcement block was active.
- **Automatic Recovery:**
  - Start Wowie again: `wowie run` or `wowie status`. On startup, Wowie automatically scans `/etc/hosts` for stale demarcation blocks and removes them safely.
- **Manual Emergency Recovery:**
  - View current blocks in `/etc/hosts`:
    ```bash
    grep -n "WOWIE" /etc/hosts
    ```
  - Remove all Wowie demarcation blocks:
    ```bash
    sudo sed -i '/# --- BEGIN WOWIE TEMPORARY DISTRACTION BLOCKLIST ---/,/# --- END WOWIE TEMPORARY DISTRACTION BLOCKLIST ---/d' /etc/hosts
    ```

---

## 5. Character Import & 3D Model Errors

### Issue: "Invalid magic bytes in glTF header" or "File too large"
- **Cause:** The uploaded model is corrupted, not a valid binary glTF/VRM file (e.g. text JSON `.gltf` with missing external buffers), or exceeds the 50 MiB security ceiling.
- **Resolution:**
  - Ensure the character model is exported as a single self-contained binary `.vrm` or `.glb` file.
  - Check file size: `ls -lh model.vrm`.
  - Inspect model metadata via CLI:
    ```bash
    wowie character import /path/to/character.vrm "Character Name"
    wowie character 3d-info
    ```

---

## 6. Local Storage & Database Reset

### Issue: Database migration corruption or corrupted local state
- **Locations:**
  - SQLite Database: `~/.local/share/wowie/wowie.db`
  - Settings: `~/.config/wowie/settings.json`
- **Recovery:**
  - Backup existing database:
    ```bash
    cp ~/.local/share/wowie/wowie.db ~/wowie_backup.db
    ```
  - Reset to a fresh state:
    ```bash
    rm ~/.local/share/wowie/wowie.db
    wowie run
    ```
