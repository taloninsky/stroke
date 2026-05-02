//! Stroke — v0.1 entry point.
//!
//! See `docs/spec.md` for the full requirements (D1–D7).

mod app;
// Scaffold-only: the `document` module's types are exercised by unit
// tests but not yet by the UI. The `capture`/`render`/`persist`
// modules added in following steps will use them; remove this allow
// at that point.
#[allow(dead_code)]
mod document;

fn main() {
    dioxus::launch(app::App);
}
