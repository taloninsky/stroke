//! Two-canvas renderer (D4).
//!
//! v0.1 responsibilities:
//! - Own the committed and live `<canvas>` elements after the app
//!   shell mounts them.
//! - Size both canvases for the device pixel ratio and scale their
//!   2D contexts so all draw calls take CSS-pixel coordinates.
//! - Repaint the committed canvas from a `Document` (full repaint;
//!   per D4 we do not optimize this in v0.1).
//! - Append a single segment of the in-progress stroke to the live
//!   canvas on each new point, and clear the live canvas when a
//!   stroke completes.
//!
//! The renderer holds no application state of its own — strokes,
//! the document, and the dirty flag live elsewhere. It is a pure
//! drawing layer that other modules (capture, persist) call into.

use std::cell::RefCell;

use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

use crate::document::{Document, Point, Stroke};

/// The two canvases and their 2D contexts. Created once at startup
/// when `init()` succeeds; thereafter accessed via [`with_state`].
struct RenderState {
    committed: CanvasRenderingContext2d,
    live: CanvasRenderingContext2d,
    /// CSS-pixel size of the canvases. Cached so we can clear by
    /// `clearRect(0, 0, width, height)` without re-querying the DOM.
    css_width: f64,
    css_height: f64,
}

thread_local! {
    /// Module-level handle to the renderer. `None` until `init()`
    /// runs; remains `Some` for the lifetime of the page.
    ///
    /// `RefCell` is acceptable here because WASM is single-threaded
    /// and every borrow in this module is short-lived (one synchronous
    /// draw operation). Re-entrancy would be a bug; if it ever
    /// happens, the panic will be loud and immediate.
    static STATE: RefCell<Option<RenderState>> = const { RefCell::new(None) };
}

/// Errors raised when wiring up the renderer.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("no `window` available")]
    NoWindow,
    #[error("no `document` available")]
    NoDocument,
    #[error("element `#{0}` not found")]
    ElementNotFound(&'static str),
    #[error("element `#{0}` is not a <canvas>")]
    NotACanvas(&'static str),
    #[error("could not obtain a 2D context for `#{0}`")]
    NoContext(&'static str),
}

/// Initialize the renderer.
///
/// Looks up the two `<canvas>` elements declared by the app shell,
/// sizes them for the current device pixel ratio, and caches their
/// 2D contexts. Idempotent: calling `init()` again replaces the
/// existing state (useful if we ever add a window-resize handler;
/// not exercised in v0.1).
pub fn init() -> Result<(), RenderError> {
    let window = web_sys::window().ok_or(RenderError::NoWindow)?;
    let document = window.document().ok_or(RenderError::NoDocument)?;

    let committed_el = canvas_by_id(&document, "stroke-committed")?;
    let live_el = canvas_by_id(&document, "stroke-live")?;

    let dpr = window.device_pixel_ratio().max(1.0);
    // We measure the *parent container* (the `relative flex-1`
    // wrapper) rather than the canvas itself. The canvas has
    // `position: absolute; inset: 0` so it inherits the parent's
    // layout box, but the canvas's own `getBoundingClientRect()` can
    // be stale or zero-sized depending on when in the frame
    // lifecycle we run. The flex parent is reliably sized once
    // Dioxus's commit + browser layout has happened.
    let parent = committed_el
        .parent_element()
        .ok_or(RenderError::ElementNotFound("stroke-committed parent"))?;
    let rect = parent.get_bounding_client_rect();
    let css_w = rect.width().max(1.0);
    let css_h = rect.height().max(1.0);

    size_canvas_to(&committed_el, css_w, css_h, dpr);
    size_canvas_to(&live_el, css_w, css_h, dpr);

    let committed = context_2d(&committed_el, "stroke-committed")?;
    let live = context_2d(&live_el, "stroke-live")?;

    // Scale once so all subsequent draw calls take CSS pixels.
    // `unwrap()` here is acceptable: this scale call cannot fail
    // for finite positive arguments (which `dpr.max(1.0)` guarantees).
    committed.scale(dpr, dpr).expect("scale committed ctx");
    live.scale(dpr, dpr).expect("scale live ctx");

    STATE.with(|s| {
        *s.borrow_mut() = Some(RenderState {
            committed,
            live,
            css_width: css_w,
            css_height: css_h,
        });
    });

    Ok(())
}

/// Repaint the committed canvas from scratch using `document`.
///
/// Used by `persist` after a successful Open and by `capture` after
/// a stroke commits. v0.1 does not optimize this; per D4, the cost
/// of a full repaint is paid only on discrete user actions.
pub fn repaint_committed(document: &Document) {
    with_state(|state| {
        clear(&state.committed, state.css_width, state.css_height);
        for stroke in &document.strokes {
            draw_stroke_full(&state.committed, stroke);
        }
    });
}

/// Begin rendering an in-progress stroke on the live canvas.
///
/// Called by `capture` on `pointerdown`. Sets the stroke style on
/// the live context so subsequent `extend_live` calls are cheap
/// `lineTo` operations.
pub fn begin_live(stroke: &Stroke) {
    with_state(|state| {
        clear(&state.live, state.css_width, state.css_height);
        configure_stroke_style(&state.live, stroke);
    });
}

/// Append a single segment to the in-progress stroke on the live
/// canvas (D4: incremental, O(1) per `pointermove`).
///
/// `prev` and `next` are consecutive points of the current stroke.
/// The first call after `begin_live` must pass the first two points;
/// subsequent calls advance the segment by one point each.
pub fn extend_live(prev: &Point, next: &Point) {
    with_state(|state| {
        let ctx = &state.live;
        ctx.begin_path();
        ctx.move_to(prev.x, prev.y);
        ctx.line_to(next.x, next.y);
        ctx.stroke();
    });
}

/// Commit the in-progress stroke: draw it onto the committed canvas
/// in full, then clear the live canvas. Called by `capture` on
/// `pointerup`.
pub fn commit_stroke(stroke: &Stroke) {
    with_state(|state| {
        draw_stroke_full(&state.committed, stroke);
        clear(&state.live, state.css_width, state.css_height);
    });
}

/// Cancel the in-progress stroke without committing it. Called by
/// `capture` on `pointercancel` or when a stroke ends with fewer
/// than two points (a single click that produced no movement).
pub fn discard_live() {
    with_state(|state| {
        clear(&state.live, state.css_width, state.css_height);
    });
}

// ---- internal helpers ---------------------------------------------------

fn with_state<R>(f: impl FnOnce(&RenderState) -> R) -> R {
    STATE.with(|s| {
        let borrowed = s.borrow();
        let state = borrowed
            .as_ref()
            .expect("render::init() must run before any draw call");
        f(state)
    })
}

fn canvas_by_id(
    document: &web_sys::Document,
    id: &'static str,
) -> Result<HtmlCanvasElement, RenderError> {
    let el = document
        .get_element_by_id(id)
        .ok_or(RenderError::ElementNotFound(id))?;
    el.dyn_into::<HtmlCanvasElement>()
        .map_err(|_| RenderError::NotACanvas(id))
}

fn context_2d(
    canvas: &HtmlCanvasElement,
    id: &'static str,
) -> Result<CanvasRenderingContext2d, RenderError> {
    canvas
        .get_context("2d")
        .map_err(|_| RenderError::NoContext(id))?
        .ok_or(RenderError::NoContext(id))?
        .dyn_into::<CanvasRenderingContext2d>()
        .map_err(|_| RenderError::NoContext(id))
}

/// Size a canvas to a known CSS-pixel size, accounting for DPR.
///
/// We accept the CSS size as input rather than reading it from the
/// canvas because the canvas's own bounding rect can be unreliable
/// during the first paint pass — see `init()` for the parent-element
/// workaround.
fn size_canvas_to(canvas: &HtmlCanvasElement, css_w: f64, css_h: f64, dpr: f64) {
    let bitmap_w = (css_w * dpr).round() as u32;
    let bitmap_h = (css_h * dpr).round() as u32;
    canvas.set_width(bitmap_w);
    canvas.set_height(bitmap_h);
}

fn clear(ctx: &CanvasRenderingContext2d, css_w: f64, css_h: f64) {
    ctx.clear_rect(0.0, 0.0, css_w, css_h);
}

/// Apply per-stroke style so subsequent path operations on `ctx`
/// honor the stroke's color, width, and the v0.1 cap/join (D4).
fn configure_stroke_style(ctx: &CanvasRenderingContext2d, stroke: &Stroke) {
    ctx.set_stroke_style_str(&stroke.color);
    ctx.set_line_width(stroke.width);
    ctx.set_line_cap("round");
    ctx.set_line_join("round");
}

/// Draw an entire stroke as a single path. Used for committed-canvas
/// painting and for the final commit of a finished stroke; not used
/// during in-progress drawing (which is incremental — see
/// `extend_live`).
fn draw_stroke_full(ctx: &CanvasRenderingContext2d, stroke: &Stroke) {
    let mut iter = stroke.points.iter();
    let Some(first) = iter.next() else { return };
    configure_stroke_style(ctx, stroke);
    ctx.begin_path();
    ctx.move_to(first.x, first.y);
    for p in iter {
        ctx.line_to(p.x, p.y);
    }
    ctx.stroke();
}
