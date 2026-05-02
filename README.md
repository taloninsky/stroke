# Stroke

A system for rich e-ink interactions, layering, and collaboration.

Ink is treated as a first-class data type — a medium where humans (and
eventually AI agents) can express spatial intent, kept mutable and
intelligence-adjacent rather than flattened into images or PDFs.

The full v0.1 specification lives in [`docs/spec.md`](docs/spec.md).

## v0.1 status

Scaffold standing up. See [`docs/spec.md`](docs/spec.md) for what is
in and out of v0.1 and the acceptance runbook (D7).

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

For active development, run Tailwind in watch mode in a second terminal:

```powershell
.\tools\tailwindcss.exe -i .\assets\tailwind.css -o .\assets\output.css --watch
```

## Tests

```powershell
cargo test --bin stroke
```

The document-model tests cover the D1 #5 round-trip contract,
schema-version rejection, and atomic-load behavior.
