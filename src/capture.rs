//! Pointer-event capture (D2, D13).
//!
//! Translates Pointer Events on the canvas surface into `Stroke`s
//! that are appended to the active document and rendered in real
//! time. v0.2 scope:
//!
//! - Listen exclusively to Pointer Events (no mouse / touch events).
//! - Sample `x`, `y`, `t` per point and stroke-level `pointer_type`.
//!   Pressure / tilt are deliberately not recorded until target-device
//!   testing proves the browser reports meaningful values.
//! - Capture the pointer on `pointerdown` so a stroke that strays
//!   off the canvas during a drag is still tracked.
//! - End the stroke cleanly on `pointerup`, `pointercancel`, or
//!   `pointerleave`.
//!
//! The capture layer owns no rendering logic — it calls into
//! `render` for the visible feedback and into a caller-supplied
//! callback when the document changes (so the app shell can update
//! the dirty flag, etc.).

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, PointerEvent};

use crate::document::{Document, Point, PointerType, Stroke};
use crate::render;

/// Author identifier hardcoded into every captured stroke (D3).
const LOCAL_AUTHOR: &str = "local-user";
/// v0.2 default stroke color (D3, D4).
const DEFAULT_COLOR: &str = "#000000";
/// v0.2 default stroke width in CSS pixels (D3, D4).
const DEFAULT_WIDTH: f64 = 2.0;
/// v0.2 default tool (D3).
const DEFAULT_TOOL: &str = "pen";

/// Errors raised when wiring up capture.
#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("no `window` available")]
    NoWindow,
    #[error("no `document` available")]
    NoDocument,
    #[error("element `#{0}` not found")]
    ElementNotFound(&'static str),
    #[error("element `#{0}` is not a <canvas>")]
    NotACanvas(&'static str),
    #[error("failed to attach event listener `{0}`")]
    ListenerAttachFailed(&'static str),
}

/// Mutable state held while a stroke is in progress.
struct InProgress {
    stroke: Stroke,
    /// Most recent point, kept separately so `extend_live` can draw
    /// the new segment without indexing back into `stroke.points`.
    last: Point,
    /// `performance.now()` value at stroke start. Each subsequent
    /// point's `t` is computed as `event.timeStamp - started_perf`.
    started_perf: f64,
}

/// Module-level capture state. `RefCell` is acceptable because WASM
/// is single-threaded and every borrow in this module is short.
struct CaptureState {
    document: Rc<RefCell<Document>>,
    /// Invoked after each successful stroke commit so the UI can
    /// refresh the dirty flag, etc.
    on_committed: Box<dyn Fn()>,
    in_progress: Option<InProgress>,
}

/// Type alias for the boxed pointer-event closures we keep alive.
/// Extracted to satisfy `clippy::type_complexity` and to make the
/// listener-storage intent legible at a glance.
type PointerClosure = Closure<dyn FnMut(PointerEvent)>;

thread_local! {
    static STATE: RefCell<Option<CaptureState>> = const { RefCell::new(None) };
    /// Closures kept alive for the lifetime of the page so their
    /// JS-side handles remain valid. Dropping a `Closure` while it
    /// is still registered as a listener is undefined behavior on
    /// the JS side; storing them here prevents that.
    static LISTENERS: RefCell<Vec<PointerClosure>> = const { RefCell::new(Vec::new()) };
}

/// Initialize capture.
///
/// Attaches the Pointer Event listeners to the **committed** canvas
/// (the bottom of the two-canvas stack). The live canvas above it is
/// `pointer-events: none` — purely a render surface — so input
/// reaches the listener target unimpeded.
///
/// `on_committed` runs after each stroke is successfully committed
/// to the document. The app shell uses this to set the dirty flag
/// and refresh any UI that depends on document state.
pub fn init(
    document: Rc<RefCell<Document>>,
    on_committed: impl Fn() + 'static,
) -> Result<(), CaptureError> {
    let window = web_sys::window().ok_or(CaptureError::NoWindow)?;
    let dom = window.document().ok_or(CaptureError::NoDocument)?;

    let canvas = dom
        .get_element_by_id("stroke-committed")
        .ok_or(CaptureError::ElementNotFound("stroke-committed"))?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| CaptureError::NotACanvas("stroke-committed"))?;

    STATE.with(|s| {
        *s.borrow_mut() = Some(CaptureState {
            document,
            on_committed: Box::new(on_committed),
            in_progress: None,
        });
    });

    attach(&canvas, "pointerdown", on_pointerdown)?;
    attach(&canvas, "pointermove", on_pointermove)?;
    attach(&canvas, "pointerup", on_pointerup)?;
    attach(&canvas, "pointercancel", on_pointercancel)?;
    attach(&canvas, "pointerleave", on_pointerleave)?;

    Ok(())
}

// ---- listener wiring ----------------------------------------------------

fn attach(
    canvas: &HtmlCanvasElement,
    event: &'static str,
    handler: fn(PointerEvent),
) -> Result<(), CaptureError> {
    let closure: PointerClosure = Closure::wrap(Box::new(handler));
    canvas
        .add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())
        .map_err(|_| CaptureError::ListenerAttachFailed(event))?;
    LISTENERS.with(|l| l.borrow_mut().push(closure));
    Ok(())
}

// ---- handlers -----------------------------------------------------------

fn on_pointerdown(event: PointerEvent) {
    // Stroke tracks only the primary button (D2). For mouse this is
    // the left button (`button == 0`); pen reports `button == 0`
    // when the tip is down.
    if event.button() != 0 {
        return;
    }

    // Capture the pointer so motion that strays off the canvas
    // during the stroke is still delivered to this element (D2).
    if let Some(target) = event
        .target()
        .and_then(|t| t.dyn_into::<HtmlCanvasElement>().ok())
    {
        let _ = target.set_pointer_capture(event.pointer_id());
    }

    let now = performance_now();
    let (x, y) = point_in_canvas(&event);
    let point = Point { x, y, t: 0.0 };
    let stroke_id = with_state_mut(|state| state.document.borrow_mut().allocate_id());
    let stroke = Stroke {
        id: stroke_id,
        author_id: LOCAL_AUTHOR.to_string(),
        tool: DEFAULT_TOOL.to_string(),
        color: DEFAULT_COLOR.to_string(),
        width: DEFAULT_WIDTH,
        started_at: chrono::Utc::now(),
        pointer_type: PointerType::from_browser(&event.pointer_type()),
        points: vec![point.clone()],
    };

    render::begin_live(&stroke);

    with_state_mut(|state| {
        state.in_progress = Some(InProgress {
            stroke,
            last: point,
            started_perf: now,
        });
    });
}

fn on_pointermove(event: PointerEvent) {
    let (x, y) = point_in_canvas(&event);
    let next = Point {
        x,
        y,
        // `event.timeStamp` is `DOMHighResTimeStamp` (ms, fractional)
        // measured against the same epoch as `performance.now()`.
        t: event.time_stamp() - started_perf_or_now(event.time_stamp()),
    };

    // Draw and append. Collect what we need under one borrow to
    // avoid re-entering `STATE`.
    let segment = with_state_mut(|state| {
        let in_prog = state.in_progress.as_mut()?;
        let prev = in_prog.last.clone();
        in_prog.stroke.points.push(next.clone());
        in_prog.last = next.clone();
        Some(prev)
    });

    if let Some(prev) = segment {
        render::extend_live(&prev, &next);
    }
}

fn on_pointerup(event: PointerEvent) {
    if event.button() != 0 {
        return;
    }
    finish_stroke(StrokeEnd::Commit);
}

fn on_pointercancel(_event: PointerEvent) {
    finish_stroke(StrokeEnd::Cancel);
}

fn on_pointerleave(_event: PointerEvent) {
    // A `pointerleave` while a stroke is in progress ends it cleanly
    // (D2). If no stroke is in progress, this is a no-op.
    finish_stroke(StrokeEnd::Commit);
}

enum StrokeEnd {
    Commit,
    Cancel,
}

fn finish_stroke(end: StrokeEnd) {
    // Take the in-progress stroke out under a single short borrow.
    let taken = with_state_mut(|state| state.in_progress.take());
    let Some(in_progress) = taken else {
        return;
    };

    match end {
        StrokeEnd::Cancel => {
            render::discard_live();
        }
        StrokeEnd::Commit => {
            // A single click with no movement produces a one-point
            // stroke that has nothing to draw. Treat it as a cancel:
            // it would be invisible on the committed canvas (no
            // segment to render) and would clutter the document.
            // Matches D4's incremental-segment rendering model.
            if in_progress.stroke.points.len() < 2 {
                render::discard_live();
                return;
            }
            render::commit_stroke(&in_progress.stroke);
            with_state_mut(|state| {
                state.document.borrow_mut().strokes.push(in_progress.stroke);
                (state.on_committed)();
            });
        }
    }
}

// ---- helpers ------------------------------------------------------------

fn with_state_mut<R>(f: impl FnOnce(&mut CaptureState) -> R) -> R {
    STATE.with(|s| {
        let mut borrowed = s.borrow_mut();
        let state = borrowed
            .as_mut()
            .expect("capture::init() must run before any pointer event");
        f(state)
    })
}

fn performance_now() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}

/// Convert a pointer event's viewport coordinates into CSS-pixel
/// coordinates relative to the canvas the listener is attached to.
///
/// We deliberately avoid `event.offset_x/y()`. Those are computed
/// against the event's *target*, which under pointer-capture can
/// drift from the listener's element (and on some browsers can also
/// be affected by transformed ancestors). `currentTarget` always
/// points at the element the listener was registered on — for us,
/// the committed canvas. Subtracting its live `getBoundingClientRect`
/// from `clientX/Y` yields stable CSS-pixel coordinates that line up
/// with what the renderer draws (which also takes CSS pixels, since
/// the 2D context was scaled by DPR at init time).
fn point_in_canvas(event: &PointerEvent) -> (f64, f64) {
    let Some(canvas) = event
        .current_target()
        .and_then(|t| t.dyn_into::<HtmlCanvasElement>().ok())
    else {
        // Fallback: if the listener somehow fired without a canvas
        // currentTarget, use the raw client coords. Better than
        // dropping the point silently — the resulting offset will be
        // visibly wrong and easy to spot in testing.
        return (event.client_x() as f64, event.client_y() as f64);
    };
    let rect = canvas.get_bounding_client_rect();
    (
        event.client_x() as f64 - rect.left(),
        event.client_y() as f64 - rect.top(),
    )
}

/// Returns the start-of-stroke `performance.now()` value if a stroke
/// is in progress; otherwise falls back to `event_time_stamp` so the
/// computed `t` becomes 0. This is only used during `pointermove`,
/// where an absent in-progress state means "ignore."
fn started_perf_or_now(event_time_stamp: f64) -> f64 {
    STATE.with(|s| {
        s.borrow()
            .as_ref()
            .and_then(|state| state.in_progress.as_ref().map(|p| p.started_perf))
            .unwrap_or(event_time_stamp)
    })
}
