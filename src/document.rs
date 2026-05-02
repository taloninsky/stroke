//! Document model — the on-disk schema for a Stroke document.
//!
//! This module defines the in-memory Rust types and the `serde`
//! (de)serialization that produces and consumes the JSON format
//! described in `docs/spec.md`, Decision 3.
//!
//! The on-disk JSON is the contract; the in-memory representation is
//! allowed to diverge from it (richer types, computed fields) so long
//! as serialization remains schema-conformant and round-trips losslessly
//! (D1 #5).

use serde::{Deserialize, Serialize};

/// The schema identifier written to and required from every v0.1 file.
///
/// Any document with a different `schema` value is rejected on load
/// with no attempt at migration (D5: strict version validation).
pub const SCHEMA_V0_1: &str = "stroke.document/v0.1";

/// Errors raised by the document model.
///
/// Kept narrow: only the failure modes the persistence layer needs to
/// distinguish for user-facing error messages (D5: error handling).
#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    /// The bytes/string were not valid JSON at all.
    #[error("file is not valid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),

    /// The JSON parsed but its `schema` field is not what v0.1 accepts.
    #[error("unsupported schema version: {0}")]
    UnsupportedSchema(String),
}

/// A complete stroke document — the root of the on-disk JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// Versioned schema id. Must equal [`SCHEMA_V0_1`] for v0.1.
    pub schema: String,

    /// Document-local identifier. Unique within this file only;
    /// global uniqueness is the graph layer's concern (Gossamer).
    pub id: u32,

    /// Document creation timestamp (ISO 8601, UTC).
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Authoritative next-id counter for any new stroke (or future
    /// child object). Stored on the document so we never have to scan
    /// to allocate.
    pub next_id: u32,

    /// Capture-time coordinate frame. Required so the coordinates in
    /// `strokes[].points` are self-describing.
    pub canvas: Canvas,

    /// Strokes in draw order. Array order is the only ordering signal;
    /// no separate index field (D3).
    pub strokes: Vec<Stroke>,
}

impl Document {
    /// Create an empty v0.1 document with the given canvas dimensions.
    ///
    /// `id` is supplied by the caller; for v0.1 there is one document
    /// per session and a constant id (e.g. `1`) is fine.
    pub fn new(id: u32, canvas: Canvas) -> Self {
        Self {
            schema: SCHEMA_V0_1.to_string(),
            id,
            created_at: chrono::Utc::now(),
            // The first allocatable id; bump every time we hand one out.
            next_id: 1,
            canvas,
            strokes: Vec::new(),
        }
    }

    /// Allocate a new id and advance the counter.
    ///
    /// Used by capture when starting a new stroke.
    pub fn allocate_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("u32 stroke id space exhausted; revisit id type");
        id
    }

    /// Serialize to pretty-printed JSON.
    ///
    /// Pretty-printing is a save-time choice and does not affect the
    /// round-trip property (D3: round-trip is modulo whitespace and key
    /// ordering).
    pub fn to_json(&self) -> Result<String, DocumentError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Parse a document from JSON, validating the schema version.
    ///
    /// Atomic: either returns a fully-loaded `Document` or returns an
    /// error and the caller's existing state is untouched (D5).
    pub fn from_json(text: &str) -> Result<Self, DocumentError> {
        let doc: Document = serde_json::from_str(text)?;
        if doc.schema != SCHEMA_V0_1 {
            return Err(DocumentError::UnsupportedSchema(doc.schema));
        }
        Ok(doc)
    }
}

/// Capture-time coordinate frame description.
///
/// Recorded once per document so coordinates in points are unambiguous
/// even when the document is later loaded onto a different-sized canvas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Canvas {
    pub width: f64,
    pub height: f64,
    /// v0.1 only value: `"css_px"`.
    pub units: String,
    /// v0.1 only value: `"top_left"`.
    pub origin: String,
}

impl Canvas {
    /// Construct a default v0.1 canvas of the given CSS-pixel size.
    pub fn css_px(width: f64, height: f64) -> Self {
        Self {
            width,
            height,
            units: "css_px".to_string(),
            origin: "top_left".to_string(),
        }
    }
}

/// A single stroke — start to lift — with its rendering metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub id: u32,
    /// v0.1 hardcoded `"local-user"`. Recording this now avoids a
    /// migration when multi-author / AI-as-author lands (D3).
    pub author_id: String,
    /// v0.1 only value: `"pen"`.
    pub tool: String,
    /// `#RRGGBB`. v0.1 always `"#000000"`. Render reads from this field.
    pub color: String,
    /// CSS pixels. v0.1 default `2.0`. Render reads from this field.
    pub width: f64,
    /// Absolute start time of the stroke (ISO 8601, UTC).
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// At least one point. Order is draw order.
    pub points: Vec<Point>,
}

/// A single sampled point within a stroke.
///
/// `pressure`, `tiltX`, `tiltY` are reserved for v0.2+ when real stylus
/// hardware is in the loop. They are deliberately absent from v0.1
/// output — recording the synthetic values a mouse reports would
/// pollute the file (D2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Point {
    /// Canvas-local CSS pixels.
    pub x: f64,
    /// Canvas-local CSS pixels.
    pub y: f64,
    /// Milliseconds since `Stroke::started_at`. First point of a stroke
    /// is `0`.
    pub t: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_doc() -> Document {
        let mut doc = Document::new(1, Canvas::css_px(1280.0, 800.0));
        let id = doc.allocate_id();
        doc.strokes.push(Stroke {
            id,
            author_id: "local-user".to_string(),
            tool: "pen".to_string(),
            color: "#000000".to_string(),
            width: 2.0,
            started_at: doc.created_at,
            points: vec![
                Point {
                    x: 100.5,
                    y: 200.0,
                    t: 0.0,
                },
                Point {
                    x: 101.2,
                    y: 201.4,
                    t: 16.0,
                },
                Point {
                    x: 103.0,
                    y: 204.1,
                    t: 33.0,
                },
            ],
        });
        doc
    }

    /// D1 #5: save → load → save produces equivalent JSON.
    ///
    /// We compare via reparse-to-`serde_json::Value` to be invariant
    /// to whitespace and key ordering, as the spec allows.
    #[test]
    fn round_trip_preserves_document() {
        let original = sample_doc();
        let json1 = original.to_json().expect("first serialize");
        let loaded = Document::from_json(&json1).expect("first parse");
        assert_eq!(original, loaded, "round-trip must preserve all fields");
        let json2 = loaded.to_json().expect("second serialize");

        let v1: serde_json::Value = serde_json::from_str(&json1).unwrap();
        let v2: serde_json::Value = serde_json::from_str(&json2).unwrap();
        assert_eq!(v1, v2, "save→load→save must produce equivalent JSON");
    }

    #[test]
    fn empty_document_round_trips() {
        let doc = Document::new(1, Canvas::css_px(1280.0, 800.0));
        let json = doc.to_json().unwrap();
        let loaded = Document::from_json(&json).unwrap();
        assert_eq!(doc, loaded);
        assert!(loaded.strokes.is_empty());
    }

    #[test]
    fn rejects_unknown_schema_version() {
        let mut doc = sample_doc();
        doc.schema = "stroke.document/v0.9".to_string();
        let json = doc.to_json().unwrap();
        match Document::from_json(&json) {
            Err(DocumentError::UnsupportedSchema(v)) => {
                assert_eq!(v, "stroke.document/v0.9");
            }
            other => panic!("expected UnsupportedSchema, got {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_json() {
        match Document::from_json("not json at all") {
            Err(DocumentError::InvalidJson(_)) => {}
            other => panic!("expected InvalidJson, got {other:?}"),
        }
    }

    /// Atomicity (D5): a failed parse must not produce a partial Document.
    /// The Result type enforces this structurally; this test pins the
    /// behavior so a future refactor cannot regress to a partial-load
    /// pattern.
    #[test]
    fn failed_parse_returns_error_not_partial_doc() {
        let bad = r#"{ "schema": "stroke.document/v0.1", "id": 1 }"#;
        assert!(Document::from_json(bad).is_err());
    }

    #[test]
    fn allocate_id_is_monotonic() {
        let mut doc = Document::new(1, Canvas::css_px(800.0, 600.0));
        let a = doc.allocate_id();
        let b = doc.allocate_id();
        let c = doc.allocate_id();
        assert!(a < b && b < c);
        assert_eq!(doc.next_id, c + 1);
    }
}
