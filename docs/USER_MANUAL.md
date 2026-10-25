# Wowie User Manual

Welcome to **Wowie**, your customizable, goal-aware AI companion on Linux. Wowie helps you turn personal ambitions into concrete focus sessions, provides contextual reminders when you drift off-task, renders your custom 3D avatar on your desktop, and supports you in staying accountable to your work.

---

## Table of Contents

1. [Quickstart Overview](#1-quickstart-overview)
2. [Installation & Setup](#2-installation--setup)
3. [The Core Focus Workflow](#3-the-core-focus-workflow)
4. [Using the Terminal Interface (TUI)](#4-using-the-terminal-interface-tui)
5. [3D Companion Character & Desktop Overlay](#5-3d-companion-character--desktop-overlay)
6. [Speech & Audio Interaction (TTS)](#6-speech--audio-interaction-tts)
7. [Browser Extension Companion](#7-browser-extension-companion)
8. [Distraction Enforcement & Website Blocking](#8-distraction-enforcement--website-blocking)
9. [CLI Command Reference](#9-cli-command-reference)
10. [Privacy, Data Storage & Reset](#10-privacy-data-storage--reset)

---

## 1. Quickstart Overview

Wowie connects what you want to achieve with what you are doing on your desktop right now:

```
[ Set Goal ] ──▶ [ Make Commitment ] ──▶ [ Start Session ] ──▶ [ Focus with Context ] ──▶ [ Review & Celebrate ]
                                                                       │
                                                       (Distraction Detected)
                                                                       ▼
                                                          [ Contextual Intervention ]
                                                          (Resume / Break / Snooze)
```

- **Interactive TUI:** Run `wowie` or `wowie run` in any terminal.
- **Automated Demo:** Run `wowie demo` to experience the 10-step focus session workflow.
- **3D Desktop Overlay:** Run `wowie overlay` to display your 3D character in a floating transparent Wayland window.

---

## 2. Installation & Setup

### Option A: Arch Linux Package (`pacman`)

If you built or downloaded the Arch Linux package:

```bash
# Build package using makepkg (no root needed during build)
cd packaging/arch
makepkg -efd

# Install the package
sudo pacman -U wowie-0.1.0-1-x86_64.pkg.tar.zst
```

Once installed, Wowie is available from your application launcher menu or via the `wowie` command.

### Option B: Building from Source

```bash
git clone https://github.com/vijaygovindBiju/Wowie-.git
cd Wowie-
cargo build --release --locked --workspace
```

The compiled binaries will be available in `target/release/`:
- `target/release/wowie`: Main companion application (TUI, CLI, coordinator).
- `target/release/wowie-overlay`: Standalone 3D transparent companion window.
- `target/release/wowie-browser-host`: WebExtensions Native Messaging host bridge.

---

## 3. The Core Focus Workflow

### Step 1: Create an Ambition and Goal
An ambition represents a major personal aspiration (e.g., *"Become a Senior Systems Engineer"*), while a goal is a concrete project or milestone (e.g., *"Master Async Rust and Tokio"*).

```bash
wowie goal create "Master Async Rust and Tokio" "Complete workspace crates and benchmarks"
```

### Step 2: Create a Focus Commitment
A commitment specifies how long you plan to focus and your target break duration:
- **Planned Focus Duration:** Standard recommended sprints are 25 minutes (Pomodoro) or 45–60 minutes (Deep Work).
- **Allowed Break Duration:** Usually 5–10 minutes.

### Step 3: Start Your Focus Session
When you start a session, Wowie activates activity monitoring strictly for the duration of the sprint:

```bash
wowie session start
```

### Step 4: Work with Desktop Awareness
While you work, Wowie observes permitted desktop window metadata (active application class, window title, and active web domain origin).
- **Privacy Guarantee:** Screen pixels, keystrokes, password fields, and form entries are **never** recorded.
- **Window Title Redaction:** You can toggle title redaction in settings to replace all window titles with `[Redacted Title]`.

### Step 5: Handling Contextual Interventions
If your active window appears unrelated to your goal for longer than your threshold (e.g., browsing entertainment during a coding sprint), your companion intervenes via desktop notifications, visual avatar expression shifts, or TUI alerts.

You have four options:
1. **Resume Work:** Switch back to your task; Wowie resets the distraction timer.
2. **Acknowledge a Break:** Switch to a planned resting interval (`wowie session break`).
3. **Correct Assessment:** If Wowie misclassified your activity (e.g., researching a bug on a forum), submit a correction to teach Wowie that this tool is relevant to your goal.
4. **Snooze:** Suppress interventions for a configurable cooldown (e.g., 5 minutes).

### Step 6: Complete Session and Review
At the end of your sprint, complete the session and record a reflection:

```bash
wowie session complete "Implemented native messaging and passed all unit tests"
```

---

## 4. Using the Terminal Interface (TUI)

Launch the interactive terminal interface:

```bash
wowie run
```

### Navigation Keys

| Key | Destination Screen | Description |
|---|---|---|
| `1` | **Status Dashboard** | Overview of active session, elapsed time, current mood, and platform observer status |
| `2` | **Goals & Ambitions** | View, create, and manage goals and actionable commitments |
| `3` | **Focus Session Control** | Start, pause, resume, break, and complete sessions |
| `4` | **Activity & Interventions** | Real-time observation log, distraction signals, and coaching advice |
| `5` | **3D Companion Character** | Active 3D model details, mesh count, available expressions, and mood preview |
| `6` | **Distraction Enforcement** | Preview and authorize temporary website blocking rules |
| `7` | **Settings & Privacy** | Tone preferences, title redaction toggles, and notification settings |
| `8` | **Review & History** | Historical session outcomes, reflection notes, and statistics |
| `q` / `Ctrl-C` | **Quit** | Gracefully disconnects observers and exits to shell |

---

## 5. 3D Companion Character & Desktop Overlay

Wowie allows you to display a floating, transparent 3D character on your desktop that reacts in real time to your focus state.

### Launching the Overlay

```bash
wowie overlay
# Or standalone:
wowie-overlay
```

On Hyprland and Sway, the overlay opens as a borderless, floating, transparent surface that sits unobtrusively in the corner of your screen.

### Importing Custom 3D Characters (VRM 1.0 / glTF)

You can import any standard humanoid or stylized character model exported in **VRM 1.0** (`.vrm`) or binary glTF (`.glb`):

```bash
# Import a model
wowie character import ~/Downloads/my_character.vrm "Aiko"

# List imported characters
wowie character list

# Select active character by ID
wowie character select <CHARACTER_UUID>

# Inspect 3D geometry and expressions
wowie character 3d-info
```

### Supported Mood Expressions

Your companion automatically changes animation and posture depending on your focus context:
- **Idle:** Gentle hovering/bobbing animation while waiting for a session.
- **Focused:** Steady, calm posture during active coding or study.
- **Distraction Alert:** Warning posture with pulsing indicator when off-task.
- **Snoozed:** Relaxed posture while interventions are snoozed.
- **Celebration:** Joyful bouncing animation when a session is completed!

### GPU Hardware Acceleration (Vulkan / WGPU)

Wowie provides two rendering backends for 3D character display:
1. **Pure-Rust Software Rasterizer (`softbuffer`):** The default backend. Uses zero external GPU drivers, works universally across Wayland and X11, and uses only ~24% CPU of one core at 30 FPS.
2. **Vulkan Hardware Acceleration:** For complex, high-polygon VRM characters (50k+ triangles), Wowie detects Vulkan libraries (`libvulkan.so.1`) and hardware acceleration capabilities. If hardware acceleration is present, it offloads geometry transforms to your GPU while automatically falling back to software rasterization if a device error occurs.

---

## 6. Speech & Audio Interaction (TTS)

Give your 3D companion character an audible voice for focus sprint milestones, distraction alerts, and celebration!

### Supported Local Voice Engines

Wowie integrates with local Linux text-to-speech tools with zero cloud latency and full privacy:
- **Piper TTS (`piper`):** High-quality local neural text-to-speech with natural voice models.
- **espeak-ng (`espeak-ng`):** Fast, lightweight multi-lingual software speech synthesizer.
- **Speech Dispatcher (`spd-say`):** Standard Linux desktop accessibility speech service.

### Companion Voice Announcements

When voice is enabled in settings, your companion speaks short, contextual prompts:
- **Sprint Start:** *"Focus session active. Stay in the flow."*
- **Distraction Alert:** *"Hey! Stay focused on your goal."*
- **Snooze:** *"Reminder snoozed. Taking a brief pause."*
- **Celebration:** *"Congratulations! You completed your focus session."*
- **Break:** *"Enjoy your break."*

Audio playback operates asynchronously via system audio servers (`pipewire` / `pulseaudio` / `alsa`), ensuring the companion UI remains responsive.

---

## 7. Browser Extension Companion

To provide accurate web origin detection without relying on fragile window title heuristics, install the lightweight Wowie WebExtension.

### 1. Register Native Messaging Host

Run the user-level setup script (no `sudo` required):

```bash
./scripts/install-browser-host.sh
```

### 2. Load the Extension in Your Browser

- **Brave / Chrome / Chromium:**
  1. Open `chrome://extensions/`.
  2. Toggle **Developer mode** in the top-right corner.
  3. Click **Load unpacked** and select the `extensions/browser/` folder from the Wowie repository.
- **Firefox:**
  1. Open `about:debugging#/runtime/this-firefox`.
  2. Click **Load Temporary Add-on** and select `extensions/browser/manifest.json`.

### 3. Privacy Guarantees
- **Origin-Only:** Only `https://domain.tld` is reported. Paths (`/watch?v=...`), queries, and fragments are stripped inside the browser sandbox before transmission.
- **No Content Access:** Keystrokes, passwords, DOM elements, and form inputs are **never** inspected.
- **Incognito Protection:** Private and incognito browsing windows are completely ignored.

---

## 7. Distraction Enforcement & Website Blocking

Wowie provides an optional, reversible distraction blocker that temporarily blocks distracting domains in `/etc/hosts` during focus sessions.

### Safety Invariants

1. **Explicit Confirmation Required:** Wowie never blocks a website without your direct consent.
2. **Essential Tool Immunities:** Development sites (`github.com`, `crates.io`, `docs.rs`, `archlinux.org`, `stackoverflow.com`) can **never** be blocked.
3. **Demarcation Markers:** All temporary rules are written inside marked sections:
   ```
   # --- BEGIN WOWIE TEMPORARY DISTRACTION BLOCKLIST ---
   127.0.0.1 youtube.com
   # --- END WOWIE TEMPORARY DISTRACTION BLOCKLIST ---
   ```
4. **Crash Recovery:** If your machine loses power or the process terminates abnormally, starting Wowie automatically cleans up any stale blocks.
5. **Emergency Manual Recovery:**
   ```bash
   sudo sed -i '/# --- BEGIN WOWIE/,/# --- END WOWIE/d' /etc/hosts
   ```

---

## 8. CLI Command Reference

Wowie includes a full command-line interface for scripting or terminal power users:

| Subcommand | Arguments | Description |
|---|---|---|
| `wowie run` | — | Launches interactive 8-screen terminal user interface (default) |
| `wowie demo` | — | Executes automated 10-step end-to-end focus demonstration |
| `wowie overlay` | — | Launches Wayland 3D transparent companion window |
| `wowie status` | — | Displays active session ID, companion mood, and observer status |
| `wowie session start` | — | Starts a focus session for your current commitment |
| `wowie session pause` | — | Temporarily pauses active session and stops observation |
| `wowie session resume`| — | Resumes a paused focus session |
| `wowie session break` | — | Starts a designated break interval |
| `wowie session complete` | `[notes]` | Completes session and saves your reflection notes |
| `wowie goal list` | — | Lists all active and archived goals |
| `wowie goal create` | `<title> [desc]` | Creates a new goal |
| `wowie character list`| — | Lists all imported 3D character avatars |
| `wowie character import` | `<path.vrm> [name]` | Validates and imports a custom VRM 1.0 model |
| `wowie character select` | `<id>` | Sets the active character avatar |
| `wowie character 3d-info`| — | Shows vertex, mesh, and bounding box metrics for active avatar |
| `wowie tick` | — | Manually evaluates a single observation and coaching cycle |
| `wowie --version` | — | Displays version information |
| `wowie --help` | — | Displays CLI options |

---

## 9. Privacy, Data Storage & Reset

### Storage Locations (XDG Compliance)

- **Application Data:** `~/.local/share/wowie/`
  - `wowie.db`: Local SQLite database (goals, commitments, session logs, user corrections).
  - `characters/`: User-imported 3D VRM models.
  - `characters.json`: Character registry metadata.
- **Configuration:** `~/.config/wowie/`
  - `settings.json`: Coaching tone preferences, sampling rates, and privacy toggles.
- **Runtime IPC:** `/run/user/$UID/wowie/`
  - `active_browser_origin.json`: Temporary cache for active browser tab origins.

### Backing Up Your Data

```bash
cp -r ~/.local/share/wowie ~/wowie_backup_$(date +%Y%m%d)
```

### Complete Reset (Factory Clean State)

To completely remove all local history, goals, and imported models:

```bash
rm -rf ~/.local/share/wowie ~/.config/wowie /run/user/$UID/wowie
```
Starting Wowie after this command will initialize a fresh, clean database automatically.
