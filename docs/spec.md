# Stroke — Specification

This document is the living spec for Stroke. It is built incrementally, one
decision at a time. Each decision is recorded with its rationale and its
explicit non-goals so that future readers (human or AI) can understand both
what was chosen and what was deliberately deferred.

## Project thesis

Stroke is an exploration of digital ink as a first-class data type — a medium
in which humans, and eventually AI agents, can express spatial intent on a
shared surface. The long-term ambition is a turn-taking (and eventually
multi-party) inking surface that integrates with Gossamer. The short-term
goal is to learn the problem space by building the smallest useful thing,
using it for real work (Bible study, journaling), and letting that friction
shape the design.

## Guiding principles

- **Capture is separate from rendering.** The on-disk/source-of-truth
  representation of ink should be lossless with respect to what the input
  device actually reports. The runtime/render representation is allowed to
  be a projection of that.
- **Schema before features.** The JSON shape of a stroke is the most
  important artifact. Get it right (or at least argued) before building
  anything that depends on it.
- **Author every stroke.** Even in single-user v0.1, every stroke carries
  an `author_id`. Multi-author and AI-as-author are core to the thesis;
  retrofitting attribution later is painful.
- **Local-first, file-based.** Persistence is a JSON file the user owns.
  No backend until there is a concrete reason for one.
- **Hard target first.** The eventual hardware target is the Boox Tab X C
  (e-ink, low refresh). Design choices should not assume a 120 Hz raster
  display.

## Out-of-scope (project-wide, until revisited)

- Real-time multi-user collaboration / CRDTs
- Cloud sync, accounts, auth
- Owning the hardware/driver layer
- Replacing the Boox native ink app

---

## Decisions

### Decision 1 — Definition of "done" for v0.1

**Status:** Accepted.

v0.1 is complete when all of the following are true:

1. The app opens in a desktop browser. Click-and-drag with the mouse renders
   a black stroke on a white canvas in real time.
2. Multiple strokes can be drawn in a single session.
3. A **Save** button writes all strokes drawn so far to a JSON file the user
   chooses where to put.
4. A **Load** button reads a previously saved JSON file and re-renders the
   strokes exactly as they were drawn.
5. The JSON schema is documented in this spec and round-trips losslessly:
   `save → load → save` produces equivalent JSON (modulo key ordering and
   insignificant whitespace).

**Explicitly NOT in v0.1:**

- Touch or stylus input (deferred to v0.2 — Boox Tab X C)
- Pressure, tilt, timing capture (mouse cannot supply these; defer until
  real stylus hardware is in the loop)
- Color picker, layers, undo, erase
- Multiple documents, navigation, autosave
- AI integration of any kind
- Any backend or network calls

**Rationale.** This scope forces commitment to the two artifacts that will
teach the most — the JSON schema and the render loop — without spending
effort on features that depend on hardware not yet plugged in. The
lossless round-trip requirement is the one piece of "more than minimum"
that is included on purpose: without it, save/load is a demo rather than
a contract.

---

### Decision 2 — Input model

**Status:** Accepted.

**API.** The capture layer listens to **Pointer Events** exclusively
(`pointerdown`, `pointermove`, `pointerup`, `pointercancel`,
`pointerleave`). Mouse events and touch events are not used.

**Stroke boundaries.**
- A stroke begins on `pointerdown` with the **primary button** held.
- Points accumulate on each `pointermove` while the primary button is held.
- A stroke ends on `pointerup`, `pointercancel`, or `pointerleave`
  (whichever comes first). All three terminate the stroke cleanly; no
  zombie strokes if the pointer leaves the canvas.
- On `pointerdown`, the canvas calls `setPointerCapture(pointerId)` so
  movement that strays outside the canvas during a stroke is still
  reported.

**Per-point fields captured in v0.1.**
- `x`, `y` — canvas-local coordinates, CSS pixels, top-left origin,
  floating point.
- `t` — millisecond timestamp relative to the **start of that stroke**
  (i.e. the first point in a stroke has `t = 0`). Sourced from
  `event.timeStamp`.

**Per-point fields deliberately not captured in v0.1.**
- `pressure`, `tiltX`, `tiltY` — a mouse reports synthetic constants
  (0.5, 0, 0) that look real but aren't. Recording them would pollute
  the file. The schema reserves these fields as optional/absent so v0.2
  can fill them in without a schema change.
- `pointerType`, `pointerId` — useful later for multi-pointer or for
  knowing whether a stroke came from pen vs. mouse; out of scope for
  v0.1.

**Sampling rate.** No throttling, no coalesced events
(`getCoalescedEvents()`), no interpolation. We take whatever the browser
fires. If high-rate mice (e.g. 8 kHz polling) become a problem, address
it then; do not pre-optimize.

**Coordinate system.** Canvas-local CSS pixels, top-left origin, floats.
The schema records the canvas dimensions at capture time so future
re-rendering on a different-sized surface is unambiguous.

**Rationale.** Pointer Events is the unified browser API for mouse,
touch, and stylus. Writing the v0.1 capture loop against it means v0.2
on the Boox is mostly "stop ignoring the extra fields" rather than a
rewrite. Recording `t` even though we are not using it preserves
schema honesty at zero cost. Refusing to record fake pressure/tilt
values keeps the file format trustworthy.

---

### Decision 3 — Stroke schema (file format)

**Status:** Accepted.

**Scope of this decision.** This describes the **on-disk JSON format** used
for save/load. The runtime/in-memory representation is a separate concern:
Rust structs that `serde` (de)serializes against this schema. The two are
allowed to differ (e.g. in-memory may use richer time types) so long as the
round-trip property in D1 holds.

**Design context.** This file format is a tree projection of what will
eventually live in Gossamer as a graph: points are "motes" (vertices),
connections within a stroke are "strands" (edges), and strokes themselves
are higher-order motes. v0.1 does not implement any of that — but the
schema is shaped so the projection is straightforward.

#### Schema (v0.1)

```json
{
  "schema": "stroke.document/v0.1",
  "id": 1,
  "created_at": "2026-05-01T21:00:00.000Z",
  "next_id": 5,
  "canvas": {
    "width": 1280,
    "height": 800,
    "units": "css_px",
    "origin": "top_left"
  },
  "strokes": [
    {
      "id": 2,
      "author_id": "local-user",
      "tool": "pen",
      "color": "#000000",
      "width": 2.0,
      "started_at": "2026-05-01T21:00:01.234Z",
      "points": [
        { "x": 100.5, "y": 200.0, "t": 0 },
        { "x": 101.2, "y": 201.4, "t": 16 },
        { "x": 103.0, "y": 204.1, "t": 33 }
      ]
    }
  ]
}
```

#### Field reference

**Document level**

| Field        | Type      | Required | Notes |
|--------------|-----------|----------|-------|
| `schema`     | string    | yes      | Versioned schema id. v0.1 = `"stroke.document/v0.1"`. Bump on any breaking change. |
| `id`         | u32       | yes      | Document-local id. Unique within this file. |
| `created_at` | ISO 8601 string (UTC) | yes | Document creation timestamp. |
| `next_id`    | u32       | yes      | The next id to allocate when a new stroke (or future child object) is added. Authoritative; do not derive from scanning. |
| `canvas`     | object    | yes      | Capture-time coordinate frame. Required so coordinates are self-describing. |
| `strokes`    | array     | yes      | Ordered. Array order is the draw order; do not encode it elsewhere. |

**Canvas object**

| Field    | Type   | Required | Notes |
|----------|--------|----------|-------|
| `width`  | number | yes      | CSS pixels at capture time. |
| `height` | number | yes      | CSS pixels at capture time. |
| `units`  | string | yes      | v0.1 only value: `"css_px"`. |
| `origin` | string | yes      | v0.1 only value: `"top_left"`. |

**Stroke object**

| Field         | Type    | Required | Notes |
|---------------|---------|----------|-------|
| `id`          | u32     | yes      | Document-local. |
| `author_id`   | string  | yes      | v0.1 hardcoded `"local-user"`. Multi-author and AI-as-author are core; recording it now avoids a migration. |
| `tool`        | string  | yes      | v0.1 only value: `"pen"`. Reserved future values: `"highlighter"`, `"eraser"`, `"select"`. |
| `color`       | string  | yes      | `#RRGGBB`. v0.1 always `"#000000"`. Render reads from this field, not a constant. |
| `width`       | number  | yes      | CSS pixels. v0.1 default `2.0`. Render reads from this field. |
| `started_at`  | ISO 8601 string (UTC) | yes | Absolute start time of the stroke. Combined with point-relative `t`, gives absolute timing without per-point absolute timestamps. |
| `points`      | array of Point | yes | At least one point. Order is draw order. |

**Point object**

| Field      | Type   | Required | Notes |
|------------|--------|----------|-------|
| `x`        | number | yes      | Canvas-local CSS pixels. |
| `y`        | number | yes      | Canvas-local CSS pixels. |
| `t`        | number | yes      | Milliseconds since `stroke.started_at`. First point = 0. |
| `pressure` | number | reserved | **Absent in v0.1.** v0.2+ may include when stylus reports it. |
| `tiltX`    | number | reserved | **Absent in v0.1.** |
| `tiltY`    | number | reserved | **Absent in v0.1.** |

#### Decisions consciously made

- **u32 ids, not ULID/UUID.** Document-local handles only. The graph layer
  (Gossamer) will mint globally unique mote ids at projection time. u32 is
  4 bytes vs. 128 bits; meaningful when a single session has tens of
  thousands of points (and eventually we may want point-level ids too).
- **Layers deferred to v0.2.** No `layer_id` field in v0.1. Adding it is a
  schema bump, not a redesign — every existing stroke is implicitly on
  layer 0.
- **Points are Point structs, not parallel arrays.** Slightly larger on
  disk; massively easier to read, debug, and project to a graph.
- **Array order encodes draw order.** No explicit index field.
- **No bounding box per stroke.** Derivable from points; do not store
  derived data.

#### Known consequences

- **Merging documents requires id remapping.** If two documents (e.g. from
  two authors) are ever combined, their u32 id spaces will collide. The
  fix is a remap pass at merge time, regenerating ids and updating
  references. Cheap; flagged here so the future-us isn't surprised.
- **Floating-point round-trip.** "Save → load → save produces equivalent
  JSON" requires lossless float (de)serialization. Rust's `serde_json`
  round-trips `f64` losslessly; browser `JSON.parse`/`JSON.stringify` of
  the same string round-trips as well. We rely on this. If it ever bites
  us, the answer is canonical formatting at save time, not changing the
  numbers.

#### Internal vs. external representation

The on-disk JSON above is the contract. The Rust in-memory model is a set
of structs (`Document`, `Canvas`, `Stroke`, `Point`) with `serde`
attributes that produce/consume this JSON. The internal model is allowed
to diverge (e.g. richer time types, computed fields, indices) as long as
serialization remains schema-conformant and the D1 round-trip holds.

---

### Decision 4 — Rendering approach

**Status:** Accepted.

**Surface layout: two stacked canvases.**

- **Committed canvas** (bottom). Holds all completed strokes. Drawn to
  exactly once per stroke, on `pointerup`. Untouched between strokes.
- **Live canvas** (top). Same dimensions, transparent background,
  `position: absolute` over the committed canvas, CSS `pointer-events:
  none` so input still hits the layer below. Holds only the in-progress
  stroke. Cleared at `pointerdown`, drawn incrementally during
  `pointermove`, cleared at `pointerup` once the stroke commits below.

**Why two canvases.** Single-canvas-redraw-everything is fine until the
document has thousands of strokes; then per-frame cost grows with
document size and we feel it on the Boox at 9 Hz long before the laptop.
Single-canvas-append-only is fast but forecloses on undo, erase, and
retroactive smoothing. Two canvases bound the per-frame work to *the
current stroke only*, with the committed surface reset cost paid only on
discrete user actions (undo, etc., when those arrive).

**In-progress stroke drawing: incremental.** On each `pointermove`, draw a
single line segment from the previous point to the new point on the live
canvas. No clearing during the stroke, no re-rendering all points each
frame. This keeps the per-event cost O(1). Tradeoff: no in-flight
smoothing. v0.1 accepts a slightly jagged stroke as honest output of what
Pointer Events provides; smoothing is a deferred decision.

**Render-loop trigger: none.** No `requestAnimationFrame` in v0.1. Drawing
happens directly inside the `pointermove` handler. Pointer events fire at
input rate, which is the rate we want to draw at. Adding rAF buys nothing
and adds a frame of latency. When stylus / high-rate input arrives,
batching coalesced events into rAF is the standard fix; revisit then.

**Stroke styling primitives.** Every stroke renders with:

- `ctx.strokeStyle = stroke.color`
- `ctx.lineWidth = stroke.width`
- `ctx.lineCap = "round"`
- `ctx.lineJoin = "round"`

Round caps and joins are non-negotiable for handwriting. Not
configurable in v0.1.

**Render reads from data, not constants.** Even though v0.1 only ever
produces black `2.0`-width strokes, the renderer pulls those values from
the `Stroke` record. This keeps the rendering path correct from day one;
the color picker (whenever it arrives) is a UI concern, not a render
change.

**DPI / device pixel ratio handling.**

- Canvas CSS size (`style.width`, `style.height`) is the logical size in
  CSS pixels — what the user and the schema both see.
- Canvas bitmap size (`canvas.width`, `canvas.height` attributes) is set
  to `cssSize * window.devicePixelRatio` on creation and on DPR change.
- After resizing the bitmap, the 2D context is scaled with
  `ctx.scale(dpr, dpr)` so all subsequent draw calls take CSS-pixel
  coordinates.
- All input coordinates from Pointer Events, all coordinates stored in
  the schema, and all coordinates passed to draw calls are **CSS
  pixels**. DPR is invisible above the render layer.

This is done from day one because failing to do so produces a blurry
canvas on any high-DPI display, which is most laptops.

#### Out of scope for v0.1

- Smoothing or curve fitting (Catmull-Rom, Bezier).
- Variable-width / pressure-driven rendering.
- Dirty-rectangle optimization on the committed canvas.
- Off-screen canvas or Web Worker rendering.
- Canvas resize handling (window resize behavior is undefined in v0.1
  beyond the initial DPR setup; revisit when it matters).

---

### Decision 5 — Persistence (save / load mechanics)

**Status:** Accepted.

**API choice: File System Access API only.** v0.1 uses
`window.showSaveFilePicker` and `window.showOpenFilePicker` exclusively.
No anchor-download fallback, no `<input type="file">` fallback.

**Browser support reality.** FSA is supported in Chromium-family browsers
(Chrome, Edge, Opera, Boox NeoBrowser). It is **not** supported in Safari
or Firefox. v0.1 is a single-user prototype the author runs on Chromium
on a laptop and on the Boox Tab X C; the gate below is sufficient.

**Unsupported-browser gate.** On app startup, feature-detect
`window.showSaveFilePicker`. If absent, render a single message
("This browser is not supported. Use a Chromium-based browser.") and do
not mount the canvas or any controls. No half-working experience.

**Save semantics.**
- First `Save` in a session calls `showSaveFilePicker` and retains the
  returned `FileSystemFileHandle`.
- Subsequent `Save` calls write through the retained handle silently —
  no picker dialog, no extra clicks.
- After loading a file via `Open`, the loaded handle becomes the active
  save target; subsequent `Save` overwrites that file.
- "Save As" is **not** in v0.1.
- Default suggested filename in the picker: `stroke-{document.id}-{ISO-date}.json`.

**Open semantics.**
- `Open` calls `showOpenFilePicker` (single-file, JSON filter).
- Read file as text, deserialize with `serde_json` into `Document`.
- Validate `schema` field. Accept only `"stroke.document/v0.1"`. Any
  other value → reject with a visible error ("Unsupported schema
  version: X"), current document untouched.
- Atomic replacement: either the entire load succeeds and the document
  is swapped, or nothing changes. No partial state.
- On success, the opened file's handle becomes the active save target.

**Dirty-flag tracking.** A boolean `dirty` flag is true whenever there
are in-memory changes that have not been saved to the active handle.
Set to `true` when a stroke completes; set to `false` after a successful
save or load.

**Confirm-on-discard.**
- `Open` button: if `dirty == true`, prompt
  ("Opening a file will discard the current document. Continue?")
  before invoking `showOpenFilePicker`. If the document is empty or
  clean, skip the prompt.
- `New` (when it exists): same prompt. v0.1 may or may not have a `New`
  button; if absent, opening another file is the only way to discard.
- `beforeunload` event: register a handler that, while `dirty == true`,
  triggers the browser's standard "Leave site?" dialog. Removed when
  `dirty == false`.

**Error handling on Open.** All three failure modes are non-destructive:
- File is not valid JSON → error message, no state change.
- JSON does not match schema → error message, no state change.
- Schema version mismatch → error message naming the version, no state
  change.

**Error handling on Save.**
- User cancels the picker → no-op, no error.
- Write failure (rare; quota, permissions revoked) → error message,
  `dirty` remains `true`.

#### No autosave — documented sharp edge

v0.1 has no autosave. Closing the tab, navigating away, or a browser
crash will lose any unsaved strokes. The `beforeunload` guard mitigates
accidental loss but does not eliminate it. **This is a known footgun.**
Autosave (to localStorage, IndexedDB, or the active FSA handle) is a
v0.2 candidate.

#### Out of scope for v0.1

- Anchor-download / `<input type="file">` fallback for non-FSA browsers.
- Drag-and-drop file loading.
- Recently-opened-files list.
- Multi-document editing.
- "Save As" as a distinct command.
- File-on-disk change detection between saves.
- Autosave of any kind.

---

### Decision 6 — Tech stack & project layout

**Status:** Accepted.

#### Stack

- **Rust + Dioxus 0.7** (web renderer) for the application shell —
  toolbar, error / unsupported-browser surfaces, canvas mount points.
- **Raw HTML `<canvas>`** for both the committed and live drawing
  surfaces. Dioxus owns the surrounding DOM but does not manage canvas
  pixels.
- **`web-sys` + `wasm-bindgen` + `js-sys`** for direct access to Pointer
  Events, Canvas 2D, and the File System Access API. No third-party
  wrapper crates.
- **`serde` + `serde_json`** for schema (de)serialization (D3).
- **`chrono`** (with `serde` and `wasmbind` features) for ISO 8601
  timestamps in the schema.
- **Tailwind CSS** for styling. Used minimally — toolbar layout,
  unsupported-browser gate, error toasts. Included now because we
  intend to use it project-wide and adding it later is friction.
- **`dioxus-cli` (`dx`)** for build, dev server, and bundling.

#### Project layout

```
stroke/
├── Cargo.toml
├── Dioxus.toml
├── tailwind.config.js
├── input.css                # Tailwind entry
├── README.md
├── docs/
│   └── spec.md
├── src/
│   ├── main.rs              # entry, app shell, unsupported-browser gate
│   ├── app.rs               # top-level Dioxus component (toolbar + canvases)
│   ├── document.rs          # Document/Canvas/Stroke/Point + serde
│   ├── capture.rs           # Pointer Events → in-progress stroke
│   ├── render.rs            # Canvas 2D draw routines (committed + live)
│   └── persist.rs           # FSA save/load, schema validation, dirty flag
└── tests/
    └── document_roundtrip.rs   # D1 #5 round-trip contract
```

Single crate. No workspace. Module split is functional, easy to
restructure once friction reveals the right shape.

#### Canvas / Dioxus interaction pattern

Each `<canvas>` is a "managed primitive": Dioxus declares the element
with a stable id, and on mount the capture/render code grabs it via
`web-sys` (`document().get_element_by_id`) and owns it for its lifetime.
Dioxus never re-renders inside the canvas. Toolbar state (dirty flag,
active file handle, error message) lives in Dioxus signals; the
capture/persist code reads and writes those signals from the JS side
of the boundary.

#### Dependencies (initial draft)

```toml
[dependencies]
dioxus = { version = "0.7", features = ["web"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
js-sys = "0.3"
web-sys = { version = "0.3", features = [
    "HtmlCanvasElement",
    "CanvasRenderingContext2d",
    "PointerEvent",
    "FileSystemFileHandle",
    "FileSystemWritableFileStream",
    "Window",
    "Document",
    "Element",
    "Blob",
    "File",
] }
chrono = { version = "0.4", features = ["serde", "wasmbind"] }
```

The `web-sys` feature set is approximate; we add what the compiler
demands. **Known wrinkle:** the FSA bindings in `web-sys` may require
`RUSTFLAGS=--cfg=web_sys_unstable_apis` (or equivalent in
`.cargo/config.toml`). If they do, we set it; if a small hand-written
extern is cleaner, we do that. Decided when we write `persist.rs`.

#### Build / dev

- `dx serve` for local development with live reload.
- `dx build --release` for production bundles (static HTML + WASM + JS
  glue).
- Tailwind: standalone CLI watching `input.css` → `assets/output.css`,
  scanning `src/**/*.rs` for class names. Run alongside `dx serve`.
- No deployment target in v0.1. Local dev server is sufficient.

#### Testing strategy (v0.1)

- **Native `cargo test`** on the `Document` model: schema round-trip
  (D1 #5), schema-version rejection, atomic-load semantics. These run
  without a browser.
- **Manual verification** for everything browser-shaped (rendering,
  Pointer Events, FSA dialogs). Covered by D7.
- No headless WASM tests in v0.1.

#### No CI

No GitHub Actions, no pre-commit hooks. Add when something is worth
guarding.

#### Out of scope for v0.1

- Workspace / multi-crate layout.
- State-management library beyond Dioxus signals.
- npm / Vite / any JS-side build tooling other than the Tailwind CLI.
- Headless WASM testing.
- CI / CD.

---

### Decision 7 — Manual verification checklist (acceptance runbook)

**Status:** Accepted.

v0.1 ships when every step below passes in one continuous run on the
author's laptop in a Chromium browser. This runbook covers the
behavioral requirements from D1–D5 that cannot be unit-tested.

**Setup.** Run `dx serve`. Open the served URL in a Chromium browser
(Chrome or Edge) on the laptop. Use a fresh tab — no cached state.

| #   | Step | Expected |
|-----|------|----------|
| S1  | Open the same URL in Firefox or Safari (if available). | Unsupported-browser message renders; no canvas appears. |
| S2  | Back in Chrome, observe initial render. | White canvas plus toolbar with `Open` and `Save`. No console errors. |
| S3  | Click and drag once across the canvas. | Black line follows the cursor with no visible lag; line stays after release. |
| S4  | Draw five more strokes (curves, straight lines, dots from quick clicks). | Each appears, none disappear, none flicker on stroke completion. |
| S5  | Begin a stroke, drag the cursor off the canvas while still pressed, release outside, then move back. | Stroke ended cleanly when cursor left; no zombie stroke when cursor returns. |
| S6  | Zoom the browser to 150% and draw another stroke. | Line is sharp, not pixelated or blurry. Reset zoom afterward. |
| S7  | Click `Save`. | FSA save dialog appears with `stroke-{id}-{date}.json` suggested. Save to a known location. No console errors. Dirty indicator (if shown) clears. |
| S8  | Open the saved file in a text editor. | Valid JSON. Contains `"schema": "stroke.document/v0.1"`. Stroke count and `points` arrays look right. Eyeball check only. |
| S9  | Without reloading, draw one more stroke. Click `Save` again. | No dialog; silent overwrite via held handle. File on disk now contains the additional stroke. |
| S10 | Draw one more stroke (now dirty). Try to close the tab or reload. | Browser shows its standard "Leave site?" dialog. Cancel and stay. |
| S11 | With the document still dirty, click `Open`. | Confirmation prompt warns the current document will be discarded. Cancel — nothing changes. |
| S12 | Save the document (clearing dirty). Click `Open`, pick the file just saved. | Canvas clears, then re-renders all strokes exactly as before. Visual match. |
| S13 | Click `Save` immediately to write back through the freshly-loaded handle. Diff the new file against the previous saved version (eyeball in a diff tool). | Files are equivalent JSON modulo key ordering and whitespace. (Rigorous check is the unit test; this is the user-side sanity check of D1 #5.) |
| S14 | Click `Open`, pick a non-JSON file (e.g. an image). Repeat with a JSON file whose `schema` is `"stroke.document/v0.9"` (handcrafted). | Visible error in each case. Current document untouched. |
| S15 | Reload the page (no dirty changes after S13). Click `Save` immediately without drawing anything. Then `Open` it back. | Save succeeds; file contains a `Document` with an empty `strokes` array. Canvas remains blank on reopen. No errors. |

#### Out of scope for v0.1 acceptance

- Boox tablet, stylus input, pressure, tilt — all v0.2.
- Performance benchmarks. v0.1 must feel responsive on a normal laptop;
  if it does not, that is a bug to fix before declaring done, not a
  separate measurement.

---

## Implementation status (post-D7)

This section tracks how each accepted decision is realized in code, plus any deviations or wrinkles surfaced during implementation.

### D1 � Mouse round-trip

- ? Mouse ? Pointer Events flow through src/capture.rs.
- ? Save/Open via src/persist.rs.
- ?? Lossless JSON round-trip is unit-tested (document::tests); end-to-end S12/S13 still a manual runbook step.

### D2 � Pointer Events

- ? src/capture.rs listens on the committed canvas only; pointerdown/move/up/leave/cancel handled.
- ? Single-click strokes (< 2 points) discarded � would be invisible under incremental rendering.
- ? `timeStamp` from PointerEvent recorded as Point.t (ms since navigation start).

### D3 � Document schema

- ? `src/document.rs` types match the spec field-for-field; 6 unit tests cover round-trip + schema rejection.

### D4 � Two-canvas rendering

- ? `src/render.rs` owns `stroke-committed` and `stroke-live`; DPR-aware sizing + `ctx.scale`.
- ? `begin_live` / `extend_live` / `commit_stroke` / `discard_live` / `repaint_committed` are the full API; `capture` and `persist` are the only callers.

### D5 � Persistence

- ? `src/persist.rs` implements FSA-only via a tiny `inline_js` bridge (5 functions). Avoided the `web_sys_unstable_apis` cfg flag in favor of a hand-rolled, type-checked surface.
- ? Held `FileSystemFileHandle` for silent overwrite.
- ? Schema-strict load with atomic replacement (parse before mutating).
- ? `beforeunload` guard installed in `app.rs` once at mount; reads live `dirty` signal each fire.
- ? Confirm-discard prompt on Open via `window.confirm`.
- ?? v0.1 surfaces persist errors to the dev console only (spec says "error message"); a UI affordance is deferred to v0.2.

### D6 � Tech stack

- ? Single binary crate, Dioxus 0.7.5 pinned to match the installed `dx` CLI.
- ? Tailwind v4 via standalone binary in `tools/` (gitignored, fetched by `tools/fetch-tailwind.ps1`).

### D7 � Manual runbook

- ?? Pending first end-to-end pass (S1�S15) once render+capture+persist are wired together in a running `dx serve` session.

