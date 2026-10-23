# ADR-004: Permissioned Linux activity observation

- **Status:** proposed; first adapter deferred to feasibility study
- **Date:** 2026-10-24

## Context

The MVP needs to observe allowed desktop metadata. Linux includes X11 and multiple Wayland compositors. A universal reliable active-window API cannot be assumed; screenshot capture is a different and more sensitive capability.

## Alternatives considered

- X11-only global observation.
- Wayland compositor-specific API/plugin.
- XDG portals for screen capture/remote desktop.
- Optional browser extension for current tab metadata.
- User-provided manual activity or no observation.

## Recommendation

Build a platform capability interface and choose one named desktop/session only after a feasibility spike. Use compositor APIs only behind a documented adapter; use portals for the capabilities they actually mediate, not as an assumed active-window feed. Keep browser URL integration separate and optional. Include manual/no-observation behavior.

## Consequences

Support breadth may be narrow initially. Permissions and indicator behavior need environment-specific verification. Screen content remains out of the initial MVP. Titles and URLs are weak evidence and never guarantee content understanding.

## Evidence and assumptions

[XDG Desktop Portal overview](https://flatpak.github.io/xdg-desktop-portal/docs/) says portals mediate capabilities through desktop-specific backends; [ScreenCast portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html) concerns screen streams; [Wayland protocol docs](https://wayland.freedesktop.org/docs/book/Protocol.html) describe compositor/client protocol architecture. Exact active-window integration is a feasibility question.

