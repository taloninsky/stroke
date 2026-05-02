# Stroke v0.2 Requirements

**Status:** Draft
**Type:** Requirement
**Audience:** Both
**Date:** 2026-05-02

This document defines the proposed v0.2 scope for Stroke. It builds on the
completed [v0.1 specification](spec.md), which proved the first vertical slice:
draw strokes, save JSON, open JSON, and repaint the canvas.

v0.2 is not a feature expansion release. It is a trust release: make the
single-document ink loop reliable enough to use for real notes on the laptop
and to test seriously on the target Boox device.

## v0.2 Thesis

Stroke v0.2 is complete when the app is a trustworthy single-document ink
surface: strokes land under the pointer, ordinary mistakes are reversible,
file operations are explicit, errors are visible in the UI, and the app can be
tested on the target device with a low-friction laptop-hosted workflow.

## Scope Summary

| Decision | Area | Proposed v0.2 Scope |
| ---------- | ------ | --------------------- |
| D8 | Release goal | Reliability and trust over new creative features. |
| D9 | Canvas layout | Robust sizing and coordinate correctness across zoom, resize, and DevTools. |
| D10 | History | Stroke-level undo/redo for committed strokes. |
| D11 | File workflow | New, Save, Save As, Open with clear dirty/file state. |
| D12 | Error UI | In-app persistence/load errors instead of console-only reporting. |
| D13 | Schema | Bump to `stroke.document/v0.2`; no v0.1 migration requirement. |
| D14 | Target device | Low-friction Boox smoke path from laptop-hosted dev server. |
| D15 | Hosted preview | Defer Cloudflare CI/CD until after local device testing proves value. |

---

## Decision 8 - v0.2 Release Goal

**Status:** Proposed.

v0.2 should harden the v0.1 loop rather than broaden the product surface.

### D8 Requirements

- The app remains a single-document editor.
- The primary workflow remains: open app, draw ink, save JSON, open JSON.
- v0.2 must preserve the v0.1 manual happy path: draw, save, reload, open,
  repaint.
- New features must directly improve trust in that loop: coordinate accuracy,
  undo/redo, file-state clarity, visible errors, or target-device testing.

### D8 Explicit Non-Goals

- Layers.
- Color palette.
- Eraser.
- Lasso/select.
- Infinite canvas.
- Text objects.
- Shape recognition.
- PDF export.
- Cloud sync.
- Accounts or authentication.
- Multi-document library.
- Real-time collaboration.
- AI integration.

### D8 Rationale

v0.1 proved the vertical slice but exposed fragility around layout timing,
coordinate mapping, and file workflow. v0.2 should remove that fragility before
adding expressive tools that would sit on top of it.

---

## Decision 9 - Canvas Sizing and Coordinate Correctness

**Status:** Proposed.

Canvas sizing and pointer coordinate mapping must become deliberate runtime
behavior, not a startup assumption.

### D9 Requirements

- On initial load, the visible ink tip appears within 2 CSS pixels of the
  pointer/stylus position at these locations:
  - top-left drawing quadrant
  - center of the drawing surface
  - bottom-right drawing quadrant
- Browser zoom at 100%, 125%, and 150% must not introduce visible coordinate
  drift.
- Opening or closing DevTools must not break pointer-to-ink alignment.
- Resizing the browser window must resize the canvas bitmap to match the new
  CSS drawing surface.
- Existing strokes must repaint correctly after a resize.
- The renderer must not depend on a one-time startup measurement that can be
  stale before flex layout settles.

### D9 Expected Design Direction

Use a real layout observation mechanism, likely `ResizeObserver`, to make the
canvas bitmap follow the drawing surface. The current v0.1 requestAnimationFrame
polling workaround is acceptable as a stopgap but should not be the v0.2
architecture.

### D9 Manual Verification

- Draw a diagonal stroke from near top-left to near bottom-right at 100%, 125%,
  and 150% zoom. The stroke tracks the pointer throughout.
- Open DevTools, resize the browser, and draw again. The stroke remains aligned.
- Save a document, resize the window, open it, and confirm existing strokes
  repaint in the expected locations.

---

## Decision 10 - Stroke-Level Undo and Redo

**Status:** Proposed.

v0.2 adds basic history for committed strokes. This is the first user-facing
creative feature because it protects ordinary mistakes without complicating the
document model.

### D10 Requirements

- `Ctrl+Z` removes the most recently committed stroke.
- `Ctrl+Y` restores the most recently undone stroke.
- `Ctrl+Shift+Z` also restores the most recently undone stroke.
- Undo on an empty document is a no-op.
- Redo with an empty redo stack is a no-op.
- Drawing a new stroke after undo clears the redo stack.
- Undo and redo both update the dirty flag.
- Save after undo persists the currently visible stroke list, not the original
  pre-undo list.
- Open and New clear the undo and redo stacks.

### D10 Expected Design Direction

Keep history session-local. The saved JSON document should contain only the
current document state, not undo or redo history.

---

## Decision 11 - New, Save, Save As, and File State

**Status:** Proposed.

v0.1 has Open and Save. v0.2 should make file ownership and dirty state explicit.

### D11 Requirements

- Add a `New` command.
- `New` creates a blank document.
- If the current document is dirty, `New` prompts before discarding changes.
- `Save` writes to the active file handle when one exists.
- `Save` prompts for a file path when no active file handle exists.
- Add a `Save As` command that always prompts for a file path.
- After successful `Save As`, the selected file handle becomes the active file
  handle.
- `Open` prompts before discarding changes when dirty.
- After successful `Open`, the opened file handle becomes the active file
  handle.
- Dirty state clears after successful Save, Save As, Open, or New.
- Dirty state remains true after failed Save or Save As.
- The toolbar/status text distinguishes at least these states:
  - unsaved new document
  - clean saved document
  - dirty saved document
- Clean and dirty saved-document states name the active file that `Save` will
  overwrite.

### D11 Deferred

- Recently opened files.
- File-on-disk change detection.
- Multi-document tabs.
- Autosave.

---

## Decision 12 - User-Visible Error Surface

**Status:** Proposed.

v0.1 reports persistence errors to the developer console. v0.2 must show errors
inside the app.

### D12 Requirements

- Invalid JSON on Open shows an in-app error message.
- Unsupported schema on Open shows an in-app error message that includes the
  schema value found in the file.
- Save failure shows an in-app error message.
- Save failure leaves the dirty flag true.
- Canceling Open, Save, or Save As does not show an error.
- A successful Open, Save, Save As, or New clears any previous persistence
  error.
- The user can dismiss the error message without changing document state.

### D12 Expected Design Direction

A small toolbar or status-area error is sufficient. v0.2 does not need a full
toast system or modal framework.

---

## Decision 13 - v0.2 Document Schema

**Status:** Proposed.

v0.2 may break compatibility with v0.1 saved files. There is no migration
requirement because no meaningful v0.1 documents need preservation.

### D13 Requirements

- The schema string becomes `stroke.document/v0.2`.
- v0.2 documents save with `"schema": "stroke.document/v0.2"`.
- Loading a v0.1 document is not required.
- If a v0.1 document is opened, the app may reject it with the same unsupported
  schema path used for any other unsupported schema.
- Unit tests cover v0.2 round-trip serialization.
- Unit tests cover unsupported schema rejection.

### D13 Pointer Metadata Scope

v0.2 should decide whether pointer metadata enters the saved schema. Proposed
minimum:

- Stroke-level `pointer_type`: `mouse`, `pen`, `touch`, or `unknown`.
- No fake pressure for mouse input.
- Pressure capture is allowed only if the browser reports meaningful pressure
  for the input device.

### D13 Accepted v0.2 Scope

v0.2 includes stroke-level `pointer_type` only. Pressure remains excluded until
Boox or other target-device testing proves the browser reports meaningful
pressure values.

---

## Decision 14 - Target Device Smoke Path

**Status:** Proposed.

The first Boox test path should be local and low ceremony: serve the app from
the development laptop on the local network, then open it from the Boox browser.

### D14 Requirements

- The app can be served from the laptop on a LAN-visible address.
- The Boox can open the laptop-hosted URL in a Chromium-based browser.
- If the Boox browser lacks File System Access API support, the unsupported
  browser gate appears and the limitation is documented.
- If the Boox browser supports the app, the user can draw at least five strokes
  using the available input method.
- Pointer/stylus-to-ink alignment is acceptable across the visible canvas.
- Save and Open are tested on the Boox.
- Any Boox-specific FSA limitation is documented before v0.2 ships.

### D14 Suggested Local Test Command

```powershell
dx serve --platform web --addr 0.0.0.0 --port 8080
```

Then open this URL on the Boox, replacing the IP with the laptop's LAN IP:

```text
http://192.168.x.y:8080
```

### D14 Deferred

- Public deployment URL.
- Cloudflare Pages CI/CD.
- Offline/PWA install.
- Performance tuning beyond obvious blockers.

---

## Decision 15 - Hosted Preview and CI/CD

**Status:** Proposed.

Cloudflare Pages or a similar hosted preview is useful, but it should follow the
first local Boox smoke test rather than precede it.

### D15 Requirements For Deciding

Choose one of these paths after the local Boox test:

| Option | When To Choose | Tradeoff |
| -------- | ---------------- | ---------- |
| Local LAN only | Boox testing works and iteration speed matters most. | No stable URL. |
| Temporary tunnel | LAN access is awkward but full CI/CD is premature. | Ephemeral URL, external dependency. |
| Cloudflare Pages | The app needs a stable repeatable preview URL. | Requires CI/CD setup and deployment maintenance. |

### D15 Deferred Until Chosen

- GitHub Actions workflow.
- Cloudflare Pages build settings.
- Release preview URLs.
- Deployment status badges.
- Public hosting documentation.

---

## Proposed v0.2 Acceptance Runbook

v0.2 ships when these pass in one continuous laptop run and, where available,
one target-device run.

| # | Step | Expected |
| --- | ------ | ---------- |
| V2-1 | Open app in laptop Chromium. | Canvas and toolbar render; no console errors. |
| V2-2 | Draw strokes near top-left, center, and bottom-right. | Ink tracks pointer within 2 CSS pixels. |
| V2-3 | Repeat V2-2 at 125% and 150% browser zoom. | No visible drift. |
| V2-4 | Resize browser and open/close DevTools. Draw again. | Canvas remains aligned; no stretched bitmap artifacts. |
| V2-5 | Draw three strokes, press `Ctrl+Z` twice. | Last two strokes disappear. |
| V2-6 | Press `Ctrl+Y`, then draw a new stroke. | One stroke returns; redo stack clears after new stroke. |
| V2-7 | Save As to a new file. | Picker appears; dirty state clears; file uses v0.2 schema; status names the file. |
| V2-8 | Draw another stroke and Save. | No picker appears; active named file is overwritten. |
| V2-9 | New while dirty. | Confirmation appears; cancel preserves document; confirm clears document. |
| V2-10 | Open invalid JSON. | In-app error appears; current document unchanged. |
| V2-11 | Open unsupported schema JSON. | In-app error names schema; current document unchanged. |
| V2-12 | Serve app on laptop LAN and open from Boox. | App loads or unsupported-browser gate appears. |
| V2-13 | If Boox is supported, draw and save/open on device. | Basic ink loop works; limitations documented. |

VS Code Simple Browser is not an acceptance browser for Save/Open. It may expose
the save picker but block `FileSystemFileHandle.createWritable`; use Chrome or
Edge for File System Access verification.

## Open Questions

1. Should v0.2 add a visible `Save As` button immediately, or hide it behind a
   compact file menu? My recommendation: visible button for now, because v0.2
   is still a tool surface and discoverability matters more than toolbar polish.
2. What exact Boox browser should be the acceptance target: NeoBrowser, Chrome,
  or whichever Chromium-based browser is easiest to install?
3. Should target-device testing be required before tagging v0.2.0, or can it be
  documented as an exploratory v0.2 milestone with a follow-up v0.2.1 hardening
  release?
