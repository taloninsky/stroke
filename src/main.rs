//! Stroke — v0.1 entry point.
//!
//! See `docs/spec.md` for the full requirements (D1–D7).

mod app;
mod capture;
mod document;
mod persist;
mod render;

fn main() {
    dioxus::launch(app::App);
}
