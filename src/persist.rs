//! Persistence — File System Access API only (D5).
//!
//! v0.2 contract:
//!
//! - Save uses `window.showSaveFilePicker` on first use; the returned
//!   file handle is held for the rest of the session so subsequent
//!   saves overwrite silently.
//! - Save As always prompts and replaces the active file handle only after a
//!   successful write.
//! - Open uses `window.showOpenFilePicker`, validates the schema
//!   strictly, and replaces the in-memory document atomically. The
//!   loaded handle becomes the active save target.
//! - The dirty flag is mirrored from the app shell so `beforeunload`
//!   and the discard prompt on Open can read it.
//!
//! The browser-side FSA glue lives as a small `inline_js` module
//! attached via `wasm-bindgen`. This avoids the `web_sys_unstable_apis`
//! cfg flag and gives us a tiny, type-checked surface across the
//! Rust/JS boundary.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;

use crate::document::{Document, DocumentError};
use crate::render;

/// Errors surfaced to the UI by save/load operations.
#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    /// The user cancelled the save or open picker. Treated as a
    /// silent no-op by the caller (D5: cancel is not an error).
    #[error("user cancelled")]
    Cancelled,
    /// The selected file failed to parse or violated the schema.
    /// Wrapped from the `document` module so the UI can show a
    /// specific message (D5: error handling).
    #[error("{0}")]
    Document(#[from] DocumentError),
    /// Anything the JS side reported that we did not classify above.
    /// Includes write failures, permission revocation, etc.
    #[error("{0}")]
    Js(String),
}

/// Module-level persistence state. Holds the active file handle (if
/// any) so the second save in a session can overwrite without
/// prompting (D5).
struct PersistState {
    document: Rc<RefCell<Document>>,
    /// `Some` once a save or open has succeeded; `None` for a fresh
    /// session before the user has chosen a file.
    handle: Option<JsValue>,
}

thread_local! {
    static STATE: RefCell<Option<PersistState>> = const { RefCell::new(None) };
}

/// Initialize the persist module with the shared document handle.
///
/// Must be called after `capture::init` so both modules share the
/// same `Document` cell.
pub fn init(document: Rc<RefCell<Document>>) {
    STATE.with(|s| {
        *s.borrow_mut() = Some(PersistState {
            document,
            handle: None,
        });
    });
}

/// Save the current document.
///
/// First call shows the FSA save picker; subsequent calls write
/// through the held handle silently (D5). On success the dirty flag
/// should be cleared by the caller. Returns the active file name on success so
/// the UI can identify what future Save operations will overwrite.
pub async fn save() -> Result<String, PersistError> {
    let json = with_state(|state| state.document.borrow().to_json())?;
    let handle = current_handle();

    let active_handle = match handle {
        Some(h) => h,
        None => {
            let suggested = with_state(|state| {
                let doc = state.document.borrow();
                suggested_filename(&doc)
            });
            match fsa_show_save_picker(&suggested).await {
                Ok(h) if h.is_undefined() || h.is_null() => {
                    return Err(PersistError::Cancelled);
                }
                Ok(h) => h,
                Err(e) => return Err(classify(e)),
            }
        }
    };

    fsa_write_text(&active_handle, &json)
        .await
        .map_err(classify)?;

    let file_name = handle_name(&active_handle).unwrap_or_else(|| "selected file".to_string());
    set_handle(active_handle);
    Ok(file_name)
}

/// Save the current document to a newly selected file.
///
/// Unlike [`save`], this always prompts for a destination. The selected handle
/// becomes active only after the write succeeds, so a failed Save As cannot
/// accidentally retarget later Save operations.
pub async fn save_as() -> Result<String, PersistError> {
    let json = with_state(|state| state.document.borrow().to_json())?;
    let suggested = with_state(|state| {
        let doc = state.document.borrow();
        suggested_filename(&doc)
    });
    let handle = match fsa_show_save_picker(&suggested).await {
        Ok(h) if h.is_undefined() || h.is_null() => {
            return Err(PersistError::Cancelled);
        }
        Ok(h) => h,
        Err(e) => return Err(classify(e)),
    };

    fsa_write_text(&handle, &json).await.map_err(classify)?;

    let file_name = handle_name(&handle).unwrap_or_else(|| "selected file".to_string());
    set_handle(handle);
    Ok(file_name)
}

/// Open a document from disk. Replaces the current document
/// atomically on success; on any failure the existing document is
/// untouched (D5).
///
/// Returns the opened file name after the new document has been swapped in and
/// the committed canvas has been repainted from it. The caller should clear the
/// dirty flag and use the name to identify the current save target.
pub async fn open() -> Result<String, PersistError> {
    let handle = match fsa_show_open_picker().await {
        Ok(h) if h.is_undefined() || h.is_null() => {
            return Err(PersistError::Cancelled);
        }
        Ok(h) => h,
        Err(e) => return Err(classify(e)),
    };

    let text = fsa_read_text(&handle).await.map_err(classify)?;
    let text = text
        .as_string()
        .ok_or_else(|| PersistError::Js("file contents were not a string".to_string()))?;

    // Parse + validate first; only mutate state if it succeeds.
    // This is the atomicity guarantee from D5: a failed parse
    // leaves the existing document in place.
    let new_doc = Document::from_json(&text)?;

    with_state(|state| {
        *state.document.borrow_mut() = new_doc;
    });
    let file_name = handle_name(&handle).unwrap_or_else(|| "selected file".to_string());
    set_handle(handle);

    // Repaint from the newly-loaded document so the user sees what
    // they opened (D5: round-trip Open).
    with_state(|state| {
        let doc = state.document.borrow();
        render::repaint_committed(&doc);
    });

    Ok(file_name)
}

/// Clear the active file handle.
///
/// Used by New, which creates an unsaved document that should prompt on the
/// next Save even if the previous session had an active file.
pub fn forget_handle() {
    STATE.with(|s| {
        if let Some(state) = s.borrow_mut().as_mut() {
            state.handle = None;
        }
    });
}

// ---- internal helpers ---------------------------------------------------

fn with_state<R>(f: impl FnOnce(&PersistState) -> R) -> R {
    STATE.with(|s| {
        let borrowed = s.borrow();
        let state = borrowed
            .as_ref()
            .expect("persist::init() must run before save/open");
        f(state)
    })
}

fn current_handle() -> Option<JsValue> {
    STATE.with(|s| s.borrow().as_ref().and_then(|state| state.handle.clone()))
}

fn set_handle(handle: JsValue) {
    STATE.with(|s| {
        if let Some(state) = s.borrow_mut().as_mut() {
            state.handle = Some(handle);
        }
    });
}

fn handle_name(handle: &JsValue) -> Option<String> {
    js_sys::Reflect::get(handle, &JsValue::from_str("name"))
        .ok()
        .and_then(|value| value.as_string())
        .filter(|name| !name.is_empty())
}

fn suggested_filename(doc: &Document) -> String {
    // D5 default pattern: stroke-{id}-{ISO date}.json. We use only
    // the date portion of the timestamp so filenames stay short and
    // sortable.
    let date = doc.created_at.format("%Y-%m-%d");
    format!("stroke-{}-{}.json", doc.id, date)
}

fn classify(err: JsValue) -> PersistError {
    // FSA reports user cancellation as an `AbortError` DOMException.
    // We unwrap that into our `Cancelled` variant so the UI does not
    // show an error dialog when the user clicked cancel.
    if let Some(name) = js_sys::Reflect::get(&err, &JsValue::from_str("name"))
        .ok()
        .and_then(|v| v.as_string())
    {
        if name == "AbortError" {
            return PersistError::Cancelled;
        }
    }
    let msg = js_sys::Reflect::get(&err, &JsValue::from_str("message"))
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| format!("{err:?}"));
    PersistError::Js(msg)
}

// ---- inline JS bridge ---------------------------------------------------
//
// The File System Access API is available on the JS `window` object
// but not yet stabilized in `web_sys`. We expose a minimal,
// type-checked surface here. Each function is `async` on the JS
// side and returns a `Promise`; `wasm-bindgen` exposes them as
// `async fn` returning `Result<JsValue, JsValue>`.

#[wasm_bindgen(inline_js = r#"
export async function fsa_show_save_picker(suggestedName) {
    const opts = {
        suggestedName,
        types: [{
            description: "Stroke document",
            accept: { "application/json": [".json"] }
        }]
    };
    return await window.showSaveFilePicker(opts);
}

export async function fsa_show_open_picker() {
    const opts = {
        multiple: false,
        types: [{
            description: "Stroke document",
            accept: { "application/json": [".json"] }
        }]
    };
    const handles = await window.showOpenFilePicker(opts);
    return handles[0];
}

export async function fsa_write_text(handle, text) {
    const writable = await handle.createWritable();
    await writable.write(text);
    await writable.close();
}

export async function fsa_read_text(handle) {
    const file = await handle.getFile();
    return await file.text();
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn fsa_show_save_picker(suggested_name: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch)]
    async fn fsa_show_open_picker() -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch)]
    async fn fsa_write_text(handle: &JsValue, text: &str) -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    async fn fsa_read_text(handle: &JsValue) -> Result<JsValue, JsValue>;
}
