# Stroke

A system for rich e-ink interactions, layering, and collaboration.

Ink is treated as a first-class data type — a medium where humans (and
eventually AI agents) can express spatial intent, kept mutable and
intelligence-adjacent rather than flattened into images or PDFs.

The v0.1 base specification lives in [`docs/spec.md`](docs/spec.md). The current
trust-release requirements live in [`docs/spec-v0.2.md`](docs/spec-v0.2.md).

## v0.2 status

Single-document ink loop with explicit `New`, `Open`, `Save`, `Save As`, active
file naming, stroke-level undo/redo, user-visible persistence errors, v0.2 JSON
schema, and runtime canvas resize handling. See the v0.2 acceptance runbook in
[`docs/spec-v0.2.md`](docs/spec-v0.2.md#proposed-v02-acceptance-runbook).

## Toolchain

- Rust + `wasm32-unknown-unknown` target
- Dioxus 0.7 (`dx` CLI, version pinned to match the `dioxus` crate)
- Tailwind CSS 4 (standalone CLI, fetched on demand — see below)

## One-time setup

```powershell
# Rust target (if not already present)
rustup target add wasm32-unknown-unknown

# Dioxus CLI (must match the `dioxus` version in Cargo.toml)
cargo install dioxus-cli --version 0.7.5 --locked

# Tailwind standalone CLI (gitignored binary, fetched into ./tools)
pwsh tools/fetch-tailwind.ps1
```

## Running the dev server

The Tailwind output is gitignored and must exist before `dx` builds,
because the `asset!` macro validates assets at compile time. Build
the CSS once, then start `dx`:

```powershell
.\tools\tailwindcss.exe -i .\assets\tailwind.css -o .\assets\output.css
dx serve --platform web
```

Open the served URL in a normal Chromium browser such as Chrome or Edge for
Save/Open testing. VS Code Simple Browser is useful for a quick render check,
but it can block File System Access writes even after showing the save picker.

For active development, run Tailwind in watch mode in a second terminal:

```powershell
.\tools\tailwindcss.exe -i .\assets\tailwind.css -o .\assets\output.css --watch
```

## LAN smoke test

For the Boox/local-device path from [Decision 14](docs/spec-v0.2.md#decision-14---target-device-smoke-path), serve on all interfaces and open the laptop's LAN URL from the device:

```powershell
dx serve --platform web --addr 0.0.0.0 --port 8080
```

Replace the IP below with the laptop's LAN address:

```text
http://192.168.x.y:8080
```

The device smoke result is not recorded yet. If the browser lacks the File
System Access API, the unsupported-browser gate is the expected result and that
limitation should be recorded before tagging v0.2.

## Tests

```powershell
cargo test --bin stroke
cargo check --target wasm32-unknown-unknown
cargo clippy --bin stroke --target wasm32-unknown-unknown
```

The native tests cover document round-trip/schema rejection and session-local
history behavior. The WASM check/clippy commands verify the browser build path
used by `dx serve`.
