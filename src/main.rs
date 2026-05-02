//! Stroke — Dioxus web entry point.
//!
//! See `docs/spec.md` for the v0.1 base and `docs/spec-v0.2.md` for the
//! current trust-release requirements.

mod app;
mod capture;
mod document;
mod history;
mod persist;
mod render;

fn main() {
    dioxus::launch(app::App);
}
